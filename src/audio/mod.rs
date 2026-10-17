//! The audio engine: owns the output stream and plays timed note lists.
//! It knows nothing about scales or the GUI — only MIDI pitches and onsets.

mod synth;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use std::sync::mpsc::{Receiver, Sender, channel};
use synth::{ScheduledNote, Synth};

/// A note in a phrase: MIDI pitch, onset in seconds, and an optional cut group (e.g. a string).
#[derive(Clone, Copy, Debug)]
pub struct PhraseNote {
    pub midi: u8,
    pub at: f32,
    pub group: Option<u8>,
}

enum Command {
    Play(Vec<PhraseNote>),
    Stop,
}

pub struct AudioEngine {
    tx: Sender<Command>,
    // Kept alive for as long as the engine exists; None if no output device.
    _stream: Option<cpal::Stream>,
    error: Option<String>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        match open_stream(rx) {
            Ok(stream) => Self { tx, _stream: Some(stream), error: None },
            Err(e) => {
                eprintln!("audio disabled: {e}");
                Self { tx, _stream: None, error: Some(e) }
            }
        }
    }

    /// Why audio is unavailable, if it is.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Start a phrase now, replacing anything still pending.
    pub fn play(&self, notes: Vec<PhraseNote>) {
        let _ = self.tx.send(Command::Play(notes));
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
                        Command::Play(notes) => {
                            synth.stop();
                            // Small offset so the first note is never late by a partial buffer.
                            let start = synth.now() + (0.01 * synth.sample_rate()) as u64;
                            let sr = synth.sample_rate();
                            synth.schedule(notes.into_iter().map(|n| ScheduledNote {
                                at_sample: start + (n.at.max(0.0) * sr) as u64,
                                midi: n.midi,
                                velocity: 0.85,
                                group: n.group,
                            }));
                        }
                    }
                }
                for frame in data.chunks_mut(channels) {
                    let s = T::from_sample(synth.next_sample());
                    frame.iter_mut().for_each(|o| *o = s);
                }
            },
            |e| eprintln!("audio stream error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}
