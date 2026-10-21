//! Karplus-Strong plucked-string synthesis, metronome clicks and a small scheduler.
//! Runs entirely on the audio thread; no allocation in steady state besides voice start.

/// What a scheduled event sounds like.
#[derive(Clone, Copy, Debug)]
pub enum Sound {
    /// A plucked string. Plucks sharing a `group` cut each other off, like two
    /// notes played on the same string.
    Pluck { midi: u8, velocity: f32, group: Option<u8> },
    /// A metronome tick; accented ones are higher and louder.
    Click { accent: bool },
}

/// A sound to start at a given sample time.
#[derive(Clone, Copy, Debug)]
pub struct Scheduled {
    pub at_sample: u64,
    pub sound: Sound,
}

struct Voice {
    delay: Vec<f32>,
    ptr: usize,
    last: f32,
    ap_coef: f32,
    ap_x1: f32,
    ap_y1: f32,
    decay: f32,
    gain: f32,
    group: Option<u8>,
    /// Extra per-sample damping once the voice is released.
    release: f32,
    age: u32,
}

impl Voice {
    fn new(sr: f32, midi: u8, velocity: f32, group: Option<u8>, seed: &mut u32) -> Self {
        let freq = 440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0);
        let period = sr / freq;
        // Total loop delay = N + 0.5 (averaging filter) + Δ (allpass), with Δ in [0.1, 1.1).
        let n = (period - 0.6).floor().max(2.0);
        let frac = period - 0.5 - n;
        let ap_coef = (1.0 - frac) / (1.0 + frac);
        let n = n as usize;
        // Excitation: a noise burst, low-passed more for softer plucks, DC removed.
        let brightness = 0.35 + 0.5 * velocity;
        let mut delay = Vec::with_capacity(n);
        let mut lp = 0.0;
        for _ in 0..n {
            *seed ^= *seed << 13;
            *seed ^= *seed >> 17;
            *seed ^= *seed << 5;
            let noise = (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            lp += brightness * (noise - lp);
            delay.push(lp);
        }
        let mean = delay.iter().sum::<f32>() / n as f32;
        delay.iter_mut().for_each(|s| *s -= mean);
        // Lower strings ring longer; decay is per loop, so scale by period.
        let t60 = 4.5 - 2.5 * ((midi as f32 - 40.0) / 48.0).clamp(0.0, 1.0);
        let decay = 10f32.powf(-3.0 * period / (t60 * sr));
        Voice {
            delay,
            ptr: 0,
            last: 0.0,
            ap_coef,
            ap_x1: 0.0,
            ap_y1: 0.0,
            decay,
            gain: 0.3 * velocity,
            group,
            release: 1.0,
            age: 0,
        }
    }

    fn next(&mut self) -> f32 {
        let x = self.delay[self.ptr];
        let lp = 0.5 * (x + self.last);
        self.last = x;
        let ap = self.ap_coef * lp + self.ap_x1 - self.ap_coef * self.ap_y1;
        self.ap_x1 = lp;
        self.ap_y1 = ap;
        self.delay[self.ptr] = ap * self.decay;
        self.ptr = (self.ptr + 1) % self.delay.len();
        self.gain *= self.release;
        self.age += 1;
        x * self.gain
    }

    fn released(&mut self, sr: f32) {
        // ~60 ms fade.
        self.release = 10f32.powf(-3.0 / (0.06 * sr));
    }

    fn finished(&self, sr: f32) -> bool {
        self.gain < 1e-4 || self.age as f32 > 8.0 * sr
    }
}

/// A short decaying sine blip.
struct Click {
    phase: f32,
    step: f32,
    gain: f32,
    decay: f32,
}

impl Click {
    fn new(sr: f32, accent: bool) -> Self {
        let freq = if accent { 1760.0 } else { 1320.0 };
        Click {
            phase: 0.0,
            step: std::f32::consts::TAU * freq / sr,
            gain: if accent { 0.45 } else { 0.3 },
            // ~25 ms to -60 dB.
            decay: 10f32.powf(-3.0 / (0.025 * sr)),
        }
    }

    fn next(&mut self) -> f32 {
        let s = self.phase.sin() * self.gain;
        self.phase = (self.phase + self.step) % std::f32::consts::TAU;
        self.gain *= self.decay;
        s
    }
}

/// A cycle of sounds repeated every `period` samples until stopped.
struct Loop {
    /// Onsets relative to the start of a cycle.
    cycle: Vec<Scheduled>,
    period: u64,
    next: u64,
}

pub struct Synth {
    sr: f32,
    clock: u64,
    voices: Vec<Voice>,
    clicks: Vec<Click>,
    pending: Vec<Scheduled>,
    looping: Option<Loop>,
    seed: u32,
    /// One-pole low-pass on the output, softening the bright pick attack.
    tone: f32,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sr: sample_rate,
            clock: 0,
            voices: Vec::with_capacity(64),
            clicks: Vec::with_capacity(8),
            pending: Vec::with_capacity(256),
            looping: None,
            seed: 0x9E37_79B9,
            tone: 0.0,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn now(&self) -> u64 {
        self.clock
    }

    pub fn schedule(&mut self, sounds: impl IntoIterator<Item = Scheduled>) {
        self.pending.extend(sounds);
    }

    /// Play `cycle` (onsets relative to its start) from sample `first`, again every `period` samples.
    pub fn repeat(&mut self, cycle: Vec<Scheduled>, first: u64, period: u64) {
        self.looping = Some(Loop { cycle, period: period.max(1), next: first });
    }

    /// Drop pending sounds and any loop, and fade out everything sounding.
    pub fn stop(&mut self) {
        self.pending.clear();
        self.looping = None;
        let sr = self.sr;
        self.voices.iter_mut().for_each(|v| v.released(sr));
    }

    pub fn next_sample(&mut self) -> f32 {
        let now = self.clock;
        if let Some(l) = &mut self.looping
            && now >= l.next
        {
            let start = l.next;
            self.pending.extend(l.cycle.iter().map(|s| Scheduled { at_sample: start + s.at_sample, ..*s }));
            l.next += l.period;
        }
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].at_sample <= now {
                let sr = self.sr;
                match self.pending.swap_remove(i).sound {
                    Sound::Pluck { midi, velocity, group } => {
                        if group.is_some() {
                            self.voices.iter_mut().filter(|v| v.group == group).for_each(|v| v.released(sr));
                        }
                        if self.voices.len() < 64 {
                            self.voices.push(Voice::new(sr, midi, velocity, group, &mut self.seed));
                        }
                    }
                    Sound::Click { accent } => {
                        if self.clicks.len() < 8 {
                            self.clicks.push(Click::new(sr, accent));
                        }
                    }
                }
            } else {
                i += 1;
            }
        }
        let mut out = 0.0;
        for v in &mut self.voices {
            out += v.next();
        }
        let mut click = 0.0;
        for c in &mut self.clicks {
            click += c.next();
        }
        let sr = self.sr;
        self.voices.retain(|v| !v.finished(sr));
        self.clicks.retain(|c| c.gain > 1e-4);
        self.clock += 1;
        self.tone += 0.55 * (out - self.tone);
        // Clicks skip the tone filter so they stay crisp.
        (self.tone * 1.2 + click).tanh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click_at(at_sample: u64) -> Scheduled {
        Scheduled { at_sample, sound: Sound::Click { accent: false } }
    }

    /// Sample indices at which a new click starts, over `n` samples.
    fn click_onsets(synth: &mut Synth, n: u64) -> Vec<u64> {
        let mut onsets = Vec::new();
        for i in 0..n {
            let before = synth.clicks.len();
            synth.next_sample();
            if synth.clicks.len() > before {
                onsets.push(i);
            }
        }
        onsets
    }

    #[test]
    fn repeat_restarts_the_cycle_every_period() {
        let mut synth = Synth::new(1000.0);
        synth.repeat(vec![click_at(0), click_at(30)], 10, 100);
        assert_eq!(click_onsets(&mut synth, 250), [10, 40, 110, 140, 210, 240]);
    }

    #[test]
    fn stop_ends_a_loop() {
        let mut synth = Synth::new(1000.0);
        synth.repeat(vec![click_at(0)], 0, 50);
        assert_eq!(click_onsets(&mut synth, 60), [0, 50]);
        synth.stop();
        assert!(click_onsets(&mut synth, 200).is_empty());
    }
}
