//! Karplus-Strong plucked-string synthesis and a small note scheduler.
//! Runs entirely on the audio thread; no allocation in steady state besides voice start.

/// A note to start at a given sample time. Notes sharing a `group` cut each
/// other off, like two notes played on the same string.
#[derive(Clone, Copy, Debug)]
pub struct ScheduledNote {
    pub at_sample: u64,
    pub midi: u8,
    pub velocity: f32,
    pub group: Option<u8>,
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

pub struct Synth {
    sr: f32,
    clock: u64,
    voices: Vec<Voice>,
    pending: Vec<ScheduledNote>,
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
            pending: Vec::with_capacity(128),
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

    pub fn schedule(&mut self, notes: impl IntoIterator<Item = ScheduledNote>) {
        self.pending.extend(notes);
    }

    /// Drop pending notes and fade out everything sounding.
    pub fn stop(&mut self) {
        self.pending.clear();
        let sr = self.sr;
        self.voices.iter_mut().for_each(|v| v.released(sr));
    }

    pub fn next_sample(&mut self) -> f32 {
        let now = self.clock;
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].at_sample <= now {
                let n = self.pending.swap_remove(i);
                let sr = self.sr;
                if n.group.is_some() {
                    self.voices.iter_mut().filter(|v| v.group == n.group).for_each(|v| v.released(sr));
                }
                if self.voices.len() < 64 {
                    self.voices.push(Voice::new(sr, n.midi, n.velocity, n.group, &mut self.seed));
                }
            } else {
                i += 1;
            }
        }
        let mut out = 0.0;
        for v in &mut self.voices {
            out += v.next();
        }
        let sr = self.sr;
        self.voices.retain(|v| !v.finished(sr));
        self.clock += 1;
        self.tone += 0.55 * (out - self.tone);
        (self.tone * 1.2).tanh()
    }
}
