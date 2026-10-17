//! User preferences, persisted as JSON.

use crate::storage;
use crate::theory::engine::{Selection, ViewKind};
use crate::theory::note::{Letter, Note};
use crate::theory::scale::{ScaleFamily, ScaleId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const FILE: &str = "settings.json";

/// What is printed inside each note marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LabelMode {
    Dots,
    Degrees,
    Notes,
}

/// Fingerboard wood, which sets the neck's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NeckWood {
    Rosewood,
    Maple,
}

impl NeckWood {
    pub const ALL: [NeckWood; 2] = [NeckWood::Rosewood, NeckWood::Maple];

    pub fn name(self) -> &'static str {
        match self {
            NeckWood::Rosewood => "Rosewood",
            NeckWood::Maple => "Maple",
        }
    }
}

/// One colour per semitone above the root (1, b2, 2, b3, 3, 4, #4, 5, b6, 6, b7, 7).
pub const DEFAULT_COLORS: [[u8; 3]; 12] = [
    [0xE5, 0x39, 0x35], // 1    red
    [0x8D, 0x6E, 0x63], // b2   brown
    [0x43, 0xA0, 0x47], // 2    green
    [0xFB, 0x8C, 0x00], // b3   orange
    [0xFD, 0xD8, 0x35], // 3    yellow
    [0x00, 0xAC, 0xC1], // 4    cyan
    [0x7E, 0x57, 0xC2], // #4   violet
    [0x1E, 0x88, 0xE5], // 5    blue
    [0x5C, 0x6B, 0xC0], // b6   indigo
    [0xEC, 0x40, 0x7A], // 6    pink
    [0xAB, 0x47, 0xBC], // b7   purple
    [0xC0, 0xCA, 0x33], // 7    lime
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub selection: Selection,
    pub tuning: String,
    pub label_mode: LabelMode,
    pub neck: NeckWood,
    pub show_scale: bool,
    pub colors: [[u8; 3]; 12],
    pub notes_per_sec: f32,
    /// Last form chosen for each selection key, so forms come back as they were left.
    pub last_form: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            selection: Selection {
                kind: ViewKind::Arpeggio,
                root: Note::new(Letter::C, 0),
                scale: ScaleId::new(ScaleFamily::Major, 0),
                chord: "maj7".into(),
            },
            tuning: "E standard".into(),
            label_mode: LabelMode::Degrees,
            neck: NeckWood::Rosewood,
            show_scale: true,
            colors: DEFAULT_COLORS,
            notes_per_sec: 5.0,
            last_form: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        storage::load(FILE)
    }

    pub fn save(&self) {
        storage::save(FILE, self);
    }
}
