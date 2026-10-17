//! Instrument tunings. Strings are indexed from the lowest (0) to the highest.

pub const FRETS: u8 = 24;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tuning {
    pub name: &'static str,
    /// MIDI pitch of each open string, lowest string first.
    pub open: &'static [u8],
}

pub const TUNINGS: &[Tuning] = &[
    Tuning { name: "E standard", open: &[40, 45, 50, 55, 59, 64] },
    Tuning { name: "Eb standard", open: &[39, 44, 49, 54, 58, 63] },
    Tuning { name: "Drop D", open: &[38, 45, 50, 55, 59, 64] },
    Tuning { name: "DADGAD", open: &[38, 45, 50, 55, 57, 62] },
    Tuning { name: "7-string B standard", open: &[35, 40, 45, 50, 55, 59, 64] },
];

impl Tuning {
    pub fn by_name(name: &str) -> &'static Tuning {
        TUNINGS.iter().find(|t| t.name == name).unwrap_or(&TUNINGS[0])
    }

    pub fn string_count(&self) -> usize {
        self.open.len()
    }

    pub fn midi(&self, string: u8, fret: u8) -> u8 {
        self.open[string as usize] + fret
    }

    /// Index of the string playing the role of the low E in the CAGED system,
    /// if the top six strings are tuned in standard intervals.
    pub fn caged_low_string(&self) -> Option<usize> {
        let n = self.open.len();
        if n < 6 {
            return None;
        }
        let base = n - 6;
        let rel: Vec<u8> = self.open[base..].iter().map(|p| p - self.open[base]).collect();
        (rel == [0, 5, 10, 15, 19, 24]).then_some(base)
    }
}

/// Note name of a pitch class with sharps, for open-string labels.
pub fn pc_name(pc: u8) -> &'static str {
    ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"][(pc % 12) as usize]
}
