//! The audio engine: owns the output stream and plays timed phrases, optionally looped,
//! with metronome ticks. It knows nothing about scales or the GUI — only MIDI pitches and onsets.

mod synth;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use std::cell::RefCell;
use std::sync::mpsc::{Receiver, Sender, channel};
use synth::{Scheduled, Sound, Synth};

/// A note in a phrase: MIDI pitch, onset in seconds, and an optional cut group (e.g. a string).
#[derive(Clone, Copy, Debug)]
pub struct PhraseNote {
    pub midi: u8,
    pub at: f32,
    pub group: Option<u8>,
}

/// A metronome tick at `at` seconds; `accent` marks the downbeat.
#[derive(Clone, Copy, Debug)]
pub struct Tick {
    pub at: f32,
    pub accent: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Phrase {
    pub notes: Vec<PhraseNote>,
    pub ticks: Vec<Tick>,
    /// Start over every this many seconds until stopped; None plays once.
    pub loop_every: Option<f32>,
}

impl Phrase {
    /// Every note and tick, with onsets in samples from the phrase start.
    fn sounds(&self, sr: f32) -> Vec<Scheduled> {
        let at = |t: f32| (t.max(0.0) * sr) as u64;
        let notes = self.notes.iter().map(|n| Scheduled {
            at_sample: at(n.at),
            sound: Sound::Pluck { midi: n.midi, velocity: 0.85, group: n.group },
        });
        let ticks = self.ticks.iter().map(|t| Scheduled { at_sample: at(t.at), sound: Sound::Click { accent: t.accent } });
        notes.chain(ticks).collect()
    }
}

enum Command {
    Play(Phrase),
    Pluck(PhraseNote),
    Mix { guitar: f32, click: f32 },
    Stop,
}

pub struct AudioEngine {
    tx: Sender<Command>,
    stream: RefCell<StreamState>,
}

enum StreamState {
    /// Not opened yet. Browsers only allow audio to start after a user gesture,
    /// so on the web the stream is opened on the first `play`.
    Pending(Receiver<Command>),
    // Kept alive for as long as the engine exists.
    Running(#[allow(dead_code)] cpal::Stream),
    Failed(String),
}

impl AudioEngine {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let engine = Self { tx, stream: RefCell::new(StreamState::Pending(rx)) };
        if !cfg!(target_arch = "wasm32") {
            engine.ensure_stream();
        }
        engine
    }

    fn ensure_stream(&self) {
        let mut state = self.stream.borrow_mut();
        if !matches!(*state, StreamState::Pending(_)) {
            return;
        }
        let StreamState::Pending(rx) = std::mem::replace(&mut *state, StreamState::Failed(String::new())) else {
            unreachable!()
        };
        *state = match open_stream(rx) {
            Ok(stream) => StreamState::Running(stream),
            Err(e) => {
                crate::storage::log_error(&format!("audio disabled: {e}"));
                StreamState::Failed(e)
            }
        };
    }

    /// Why audio is unavailable, if it is.
    pub fn error(&self) -> Option<String> {
        match &*self.stream.borrow() {
            StreamState::Failed(e) => Some(e.clone()),
            _ => None,
        }
    }

    /// Start a phrase now, replacing anything still pending.
    pub fn play(&self, phrase: Phrase) {
        self.ensure_stream();
        let _ = self.tx.send(Command::Play(phrase));
    }

    /// Sound one note now, on top of whatever is playing.
    pub fn pluck(&self, note: PhraseNote) {
        self.ensure_stream();
        let _ = self.tx.send(Command::Pluck(note));
    }

    /// Set the guitar and metronome levels (0..=1).
    pub fn set_mix(&self, guitar: f32, click: f32) {
        let _ = self.tx.send(Command::Mix { guitar, click });
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Command::Stop);
    }
}

fn open_stream(rx: Receiver<Command>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no output device")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, rx),
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, rx),
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, rx),
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, rx),
        other => return Err(format!("unsupported sample format {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn build<T>(device: &cpal::Device, config: &cpal::StreamConfig, rx: Receiver<Command>) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut synth = Synth::new(config.sample_rate as f32);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                while let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        Command::Stop => synth.stop(),
                        Command::Mix { guitar, click } => synth.set_mix(guitar, click),
                        Command::Play(phrase) => {
                            synth.stop();
                            // Small offset so the first note is never late by a partial buffer.
                            let sr = synth.sample_rate();
                            let start = synth.now() + (0.01 * sr) as u64;
                            let sounds = phrase.sounds(sr);
                            match phrase.loop_every {
                                Some(period) => synth.repeat(sounds, start, (period * sr) as u64),
                                None => synth.schedule(
                                    sounds.into_iter().map(|s| Scheduled { at_sample: start + s.at_sample, ..s }),
                                ),
                            }
                        }
                        Command::Pluck(n) => {
                            let sound = Sound::Pluck { midi: n.midi, velocity: 0.85, group: n.group };
                            synth.schedule([Scheduled { at_sample: synth.now(), sound }]);
                        }
                    }
                }
                for frame in data.chunks_mut(channels) {
                    let s = T::from_sample(synth.next_sample());
                    frame.iter_mut().for_each(|o| *o = s);
                }
            },
            |e| crate::storage::log_error(&format!("audio stream error: {e}")),
            None,
        )
        .map_err(|e| e.to_string())
}
