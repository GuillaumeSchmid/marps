//! Chord / arpeggio qualities, from triads up to altered 13ths.

use super::note::{Interval, intervals};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChordCategory {
    Triad,
    Sixth,
    Seventh,
    Extended,
    Altered,
}

impl ChordCategory {
    pub const ALL: [ChordCategory; 5] = [
        ChordCategory::Triad,
        ChordCategory::Sixth,
        ChordCategory::Seventh,
        ChordCategory::Extended,
        ChordCategory::Altered,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ChordCategory::Triad => "Triads & sus",
            ChordCategory::Sixth => "6ths & add",
            ChordCategory::Seventh => "7ths",
            ChordCategory::Extended => "9 / 11 / 13",
            ChordCategory::Altered => "Altered dominants",
        }
    }
}

/// A chord quality, independent of root. `symbol` is the stable id used for saved shapes.
#[derive(Clone, Copy, Debug)]
pub struct ChordQuality {
    pub symbol: &'static str,
    pub name: &'static str,
    pub category: ChordCategory,
    labels: &'static [&'static str],
}

use ChordCategory::*;

const fn q(
    symbol: &'static str,
    name: &'static str,
    category: ChordCategory,
    labels: &'static [&'static str],
) -> ChordQuality {
    ChordQuality { symbol, name, category, labels }
}

pub const CHORDS: &[ChordQuality] = &[
    q("maj", "Major", Triad, &["1", "3", "5"]),
    q("m", "Minor", Triad, &["1", "b3", "5"]),
    q("dim", "Diminished", Triad, &["1", "b3", "b5"]),
    q("aug", "Augmented", Triad, &["1", "3", "#5"]),
    q("sus2", "Suspended 2nd", Triad, &["1", "2", "5"]),
    q("sus4", "Suspended 4th", Triad, &["1", "4", "5"]),
    q("(b5)", "Major flat 5", Triad, &["1", "3", "b5"]),
    q("6", "Major 6th", Sixth, &["1", "3", "5", "6"]),
    q("m6", "Minor 6th", Sixth, &["1", "b3", "5", "6"]),
    q("6/9", "Six-nine", Sixth, &["1", "3", "5", "6", "9"]),
    q("m6/9", "Minor six-nine", Sixth, &["1", "b3", "5", "6", "9"]),
    q("add9", "Add 9", Sixth, &["1", "3", "5", "9"]),
    q("madd9", "Minor add 9", Sixth, &["1", "b3", "5", "9"]),
    q("maj7", "Major 7th", Seventh, &["1", "3", "5", "7"]),
    q("7", "Dominant 7th", Seventh, &["1", "3", "5", "b7"]),
    q("m7", "Minor 7th", Seventh, &["1", "b3", "5", "b7"]),
    q("m(maj7)", "Minor-major 7th", Seventh, &["1", "b3", "5", "7"]),
    q("m7b5", "Half-diminished", Seventh, &["1", "b3", "b5", "b7"]),
    q("dim7", "Diminished 7th", Seventh, &["1", "b3", "b5", "bb7"]),
    q("maj7#5", "Major 7th sharp 5", Seventh, &["1", "3", "#5", "7"]),
    q("maj7b5", "Major 7th flat 5", Seventh, &["1", "3", "b5", "7"]),
    q("7#5", "Augmented 7th", Seventh, &["1", "3", "#5", "b7"]),
    q("7b5", "Dominant 7th flat 5", Seventh, &["1", "3", "b5", "b7"]),
    q("7sus4", "Dominant 7th sus 4", Seventh, &["1", "4", "5", "b7"]),
    q("maj9", "Major 9th", Extended, &["1", "3", "5", "7", "9"]),
    q("9", "Dominant 9th", Extended, &["1", "3", "5", "b7", "9"]),
    q("m9", "Minor 9th", Extended, &["1", "b3", "5", "b7", "9"]),
    q("m(maj9)", "Minor-major 9th", Extended, &["1", "b3", "5", "7", "9"]),
    q("9sus4", "Dominant 9th sus 4", Extended, &["1", "4", "5", "b7", "9"]),
    q("maj7#11", "Lydian major 7th", Extended, &["1", "3", "5", "7", "#11"]),
    q("11", "Dominant 11th", Extended, &["1", "5", "b7", "9", "11"]),
    q("m11", "Minor 11th", Extended, &["1", "b3", "5", "b7", "9", "11"]),
    q("m11b5", "Half-diminished 11th", Extended, &["1", "b3", "b5", "b7", "11"]),
    q("maj13", "Major 13th", Extended, &["1", "3", "5", "7", "9", "13"]),
    q("13", "Dominant 13th", Extended, &["1", "3", "5", "b7", "9", "13"]),
    q("m13", "Minor 13th", Extended, &["1", "b3", "5", "b7", "9", "11", "13"]),
    q("7b9", "Dominant 7th flat 9", Altered, &["1", "3", "5", "b7", "b9"]),
    q("7#9", "Dominant 7th sharp 9", Altered, &["1", "3", "5", "b7", "#9"]),
    q("7#11", "Lydian dominant 7th", Altered, &["1", "3", "5", "b7", "#11"]),
    q("7b13", "Dominant 7th flat 13", Altered, &["1", "3", "5", "b7", "b13"]),
    q("7b9b13", "Dominant 7th b9 b13", Altered, &["1", "3", "b7", "b9", "b13"]),
    q("7#9b13", "Dominant 7th #9 b13", Altered, &["1", "3", "b7", "#9", "b13"]),
    q("7b5b9", "Dominant 7th b5 b9", Altered, &["1", "3", "b5", "b7", "b9"]),
    q("7#5#9", "Dominant 7th #5 #9", Altered, &["1", "3", "#5", "b7", "#9"]),
    q("9#11", "Dominant 9th sharp 11", Altered, &["1", "3", "5", "b7", "9", "#11"]),
    q("13b9", "Dominant 13th flat 9", Altered, &["1", "3", "5", "b7", "b9", "13"]),
    q("13#11", "Dominant 13th sharp 11", Altered, &["1", "3", "5", "b7", "9", "#11", "13"]),
    q("7alt", "Altered dominant", Altered, &["1", "3", "b7", "b9", "#9", "#11", "b13"]),
];

impl ChordQuality {
    pub fn intervals(&self) -> Vec<Interval> {
        intervals(self.labels)
    }

    pub fn by_symbol(symbol: &str) -> Option<&'static ChordQuality> {
        CHORDS.iter().find(|c| c.symbol == symbol)
    }

    /// Display suffix after the root, e.g. "" for major, "m7b5".
    pub fn suffix(&self) -> &'static str {
        if self.symbol == "maj" { "" } else { self.symbol }
    }

    /// Tones a guitar voicing may leave out: the natural 5th, and the
    /// lower natural extensions once a higher one is present.
    pub fn is_optional(&self, interval: Interval) -> bool {
        let ivs = self.intervals();
        let has = |l: &str| ivs.iter().any(|i| i.label() == l);
        match interval.label().as_str() {
            "5" => ivs.len() > 3,
            "9" => has("11") || has("13"),
            "11" => has("13"),
            _ => false,
        }
    }

    /// The catalogue entry with exactly these intervals, if any.
    pub fn find(ivs: &[Interval]) -> Option<&'static ChordQuality> {
        let mut want: Vec<_> = ivs.to_vec();
        want.sort_by_key(|i| (i.number, i.semitones));
        CHORDS.iter().find(|c| {
            let mut have = c.intervals();
            have.sort_by_key(|i| (i.number, i.semitones));
            have == want
        })
    }
}
