//! Scale families and their modes.

use super::note::{Interval, intervals};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScaleFamily {
    Major,
    MelodicMinor,
    HarmonicMinor,
    Other,
}

impl ScaleFamily {
    pub const ALL: [ScaleFamily; 4] = [
        ScaleFamily::Major,
        ScaleFamily::MelodicMinor,
        ScaleFamily::HarmonicMinor,
        ScaleFamily::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ScaleFamily::Major => "Major (Ionian) modes",
            ScaleFamily::MelodicMinor => "Melodic minor modes",
            ScaleFamily::HarmonicMinor => "Harmonic minor modes",
            ScaleFamily::Other => "Pentatonic / blues / symmetric",
        }
    }

    pub fn mode_names(self) -> &'static [&'static str] {
        match self {
            ScaleFamily::Major => &[
                "Ionian", "Dorian", "Phrygian", "Lydian", "Mixolydian", "Aeolian", "Locrian",
            ],
            ScaleFamily::MelodicMinor => &[
                "Melodic minor",
                "Dorian b2",
                "Lydian augmented",
                "Lydian dominant",
                "Mixolydian b6",
                "Locrian #2",
                "Altered",
            ],
            ScaleFamily::HarmonicMinor => &[
                "Harmonic minor",
                "Locrian #6",
                "Ionian #5",
                "Dorian #4",
                "Phrygian dominant",
                "Lydian #2",
                "Super-Locrian bb7",
            ],
            ScaleFamily::Other => &[
                "Major pentatonic",
                "Minor pentatonic",
                "Minor blues",
                "Major blues",
                "Whole tone",
                "Diminished (half-whole)",
                "Diminished (whole-half)",
            ],
        }
    }

    /// Step pattern of the parent scale, for the heptatonic families.
    fn parent_semitones(self) -> Option<[u8; 7]> {
        match self {
            ScaleFamily::Major => Some([0, 2, 4, 5, 7, 9, 11]),
            ScaleFamily::MelodicMinor => Some([0, 2, 3, 5, 7, 9, 11]),
            ScaleFamily::HarmonicMinor => Some([0, 2, 3, 5, 7, 8, 11]),
            ScaleFamily::Other => None,
        }
    }

    /// Whether the modes are rotations of one 7-note parent (so chords can be built per degree).
    pub fn is_heptatonic(self) -> bool {
        self.parent_semitones().is_some()
    }
}

/// One concrete scale: a family plus a mode index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScaleId {
    pub family: ScaleFamily,
    pub mode: usize,
}

impl ScaleId {
    pub const fn new(family: ScaleFamily, mode: usize) -> Self {
        Self { family, mode }
    }

    pub fn name(self) -> &'static str {
        self.family.mode_names()[self.mode]
    }

    /// Every selectable scale.
    pub fn all() -> Vec<ScaleId> {
        ScaleFamily::ALL
            .iter()
            .flat_map(|&f| (0..f.mode_names().len()).map(move |m| ScaleId::new(f, m)))
            .collect()
    }

    /// Intervals of the scale above its root, in ascending order.
    pub fn intervals(self) -> Vec<Interval> {
        if let Some(parent) = self.family.parent_semitones() {
            let base = parent[self.mode];
            return (0..7)
                .map(|i| {
                    let s = (parent[(self.mode + i) % 7] + 12 - base) % 12;
                    Interval::new(i as u8 + 1, s)
                })
                .collect();
        }
        let labels: &[&str] = match self.mode {
            0 => &["1", "2", "3", "5", "6"],
            1 => &["1", "b3", "4", "5", "b7"],
            2 => &["1", "b3", "4", "b5", "5", "b7"],
            3 => &["1", "2", "b3", "3", "5", "6"],
            4 => &["1", "2", "3", "#4", "#5", "b7"],
            5 => &["1", "b2", "#2", "3", "#4", "5", "6", "b7"],
            _ => &["1", "2", "b3", "4", "b5", "#5", "6", "7"],
        };
        intervals(labels)
    }

    /// For heptatonic families: the mode that starts on this scale's `degree` (0-based).
    pub fn mode_on_degree(self, degree: usize) -> ScaleId {
        ScaleId::new(self.family, (self.mode + degree) % 7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(id: ScaleId) -> Vec<String> {
        id.intervals().iter().map(|i| i.label()).collect()
    }

    #[test]
    fn mode_intervals() {
        assert_eq!(
            labels(ScaleId::new(ScaleFamily::Major, 1)),
            ["1", "2", "b3", "4", "5", "6", "b7"]
        );
        assert_eq!(
            labels(ScaleId::new(ScaleFamily::MelodicMinor, 6)),
            ["1", "b2", "b3", "b4", "b5", "b6", "b7"]
        );
        assert_eq!(
            labels(ScaleId::new(ScaleFamily::HarmonicMinor, 4)),
            ["1", "b2", "3", "4", "5", "b6", "b7"]
        );
        assert_eq!(
            labels(ScaleId::new(ScaleFamily::HarmonicMinor, 6)),
            ["1", "b2", "b3", "b4", "b5", "b6", "bb7"]
        );
    }
}
