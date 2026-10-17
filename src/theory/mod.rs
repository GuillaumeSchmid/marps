//! Music theory: the `ScaleEngine` and everything it is built from.
//! Pure data and computation — no GUI, no audio.

pub mod caged;
pub mod chord;
pub mod engine;
pub mod note;
pub mod scale;
pub mod tuning;

use serde::{Deserialize, Serialize};

/// A position on the neck. String 0 is the lowest-pitched string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pos {
    pub string: u8,
    pub fret: u8,
}

impl Pos {
    pub const fn new(string: u8, fret: u8) -> Self {
        Self { string, fret }
    }
}
