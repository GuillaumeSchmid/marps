//! Pitch classes, spelled note names and intervals.
//!
//! Spelling is letter-based: an interval always moves a fixed number of
//! letters, so Eb major yields Bb/Ab and F# major yields E#.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The seven note letters, in order from C.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Letter {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

impl Letter {
    pub const ALL: [Letter; 7] = [
        Letter::C,
        Letter::D,
        Letter::E,
        Letter::F,
        Letter::G,
        Letter::A,
        Letter::B,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Letter {
        Self::ALL[i % 7]
    }

    /// Pitch class of the natural note.
    pub fn natural_pc(self) -> u8 {
        [0, 2, 4, 5, 7, 9, 11][self.index()]
    }

    pub fn as_str(self) -> &'static str {
        ["C", "D", "E", "F", "G", "A", "B"][self.index()]
    }
}

/// A spelled note: a letter plus an accidental (-2 = double flat … +2 = double sharp).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Note {
    pub letter: Letter,
    pub accidental: i8,
}

impl Note {
    pub const fn new(letter: Letter, accidental: i8) -> Self {
        Self { letter, accidental }
    }

    /// Pitch class 0..12 (C = 0).
    pub fn pc(self) -> u8 {
        (self.letter.natural_pc() as i16 + self.accidental as i16).rem_euclid(12) as u8
    }

    /// The note `interval` above this one, spelled by letter distance.
    pub fn transpose(self, interval: Interval) -> Note {
        let letter = Letter::from_index(self.letter.index() + (interval.number as usize - 1));
        let target = (self.pc() as i16 + interval.semitones as i16).rem_euclid(12);
        let mut acc = (target - letter.natural_pc() as i16).rem_euclid(12);
        if acc > 6 {
            acc -= 12;
        }
        Note::new(letter, acc as i8)
    }

    /// The roots offered in the root selector (common spellings only).
    pub fn selectable_roots() -> Vec<Note> {
        use Letter::*;
        vec![
            Note::new(C, 0),
            Note::new(C, 1),
            Note::new(D, -1),
            Note::new(D, 0),
            Note::new(D, 1),
            Note::new(E, -1),
            Note::new(E, 0),
            Note::new(F, 0),
            Note::new(F, 1),
            Note::new(G, -1),
            Note::new(G, 0),
            Note::new(G, 1),
            Note::new(A, -1),
            Note::new(A, 0),
            Note::new(A, 1),
            Note::new(B, -1),
            Note::new(B, 0),
        ]
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let acc = match self.accidental {
            -2 => "bb",
            -1 => "b",
            0 => "",
            1 => "#",
            2 => "x",
            n if n < 0 => return write!(f, "{}{}", self.letter.as_str(), "b".repeat(-n as usize)),
            n => return write!(f, "{}{}", self.letter.as_str(), "#".repeat(n as usize)),
        };
        write!(f, "{}{}", self.letter.as_str(), acc)
    }
}

/// An interval above a root: a degree number (1..=13) and a size in semitones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Interval {
    pub number: u8,
    pub semitones: u8,
}

impl Interval {
    pub const fn new(number: u8, semitones: u8) -> Self {
        Self { number, semitones }
    }

    /// Semitones of the major/perfect interval with this degree number.
    fn reference_semitones(number: u8) -> i16 {
        let simple = (number - 1) % 7;
        let octaves = (number - 1) / 7;
        [0, 2, 4, 5, 7, 9, 11][simple as usize] + 12 * octaves as i16
    }

    /// Parse a degree label such as "1", "b3", "#11", "bb7".
    pub fn parse(label: &str) -> Interval {
        let digits = label.trim_start_matches(['b', '#']);
        let prefix = &label[..label.len() - digits.len()];
        let number: u8 = digits.parse().expect("invalid degree label");
        let shift: i16 = prefix.chars().map(|c| if c == 'b' { -1 } else { 1 }).sum();
        let semis = Self::reference_semitones(number) + shift;
        Interval::new(number, semis as u8)
    }

    /// Degree label, e.g. "b3", "#11", "bb7". The root is "1".
    pub fn label(self) -> String {
        let diff = self.semitones as i16 - Self::reference_semitones(self.number);
        let prefix = match diff {
            d if d < 0 => "b".repeat(-d as usize),
            d => "#".repeat(d as usize),
        };
        format!("{prefix}{}", self.number)
    }

    /// Semitone class 0..12, used e.g. to pick a degree colour.
    pub fn pc(self) -> u8 {
        self.semitones % 12
    }
}

/// Parse a list of degree labels.
pub fn intervals(labels: &[&str]) -> Vec<Interval> {
    labels.iter().map(|l| Interval::parse(l)).collect()
}

/// Human name for each of the 12 semitone classes as a degree (used for colour swatches).
pub const DEGREE_CLASS_LABELS: [&str; 12] = [
    "1", "b2 / b9", "2 / 9", "b3 / #9", "3", "4 / 11", "#4 / b5", "5", "#5 / b6", "6 / 13", "b7",
    "7",
];

#[cfg(test)]
mod tests {
    use super::*;
    use Letter::*;

    #[test]
    fn spelling_follows_letters() {
        let fs = Note::new(F, 1);
        assert_eq!(fs.transpose(Interval::parse("7")).to_string(), "E#");
        let eb = Note::new(E, -1);
        assert_eq!(eb.transpose(Interval::parse("5")).to_string(), "Bb");
        assert_eq!(eb.transpose(Interval::parse("7")).to_string(), "D");
        let c = Note::new(C, 0);
        assert_eq!(c.transpose(Interval::parse("bb7")).to_string(), "Bbb");
        assert_eq!(c.transpose(Interval::parse("#11")).to_string(), "F#");
        assert_eq!(c.transpose(Interval::parse("b13")).to_string(), "Ab");
    }

    #[test]
    fn labels_round_trip() {
        for l in ["1", "b3", "#5", "bb7", "b9", "#9", "#11", "b13", "13"] {
            assert_eq!(Interval::parse(l).label(), l);
        }
    }
}
