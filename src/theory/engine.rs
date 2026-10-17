//! The `ScaleEngine`: turns a selection (root + scale / arpeggio / chord) into
//! everything the GUI draws and the audio engine plays.

use super::Pos;
use super::caged::{self, CagedPosition};
use super::chord::ChordQuality;
use super::note::{Interval, Note};
use super::scale::ScaleId;
use super::tuning::{FRETS, Tuning};
use serde::{Deserialize, Serialize};

/// What is being studied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViewKind {
    Scale,
    Arpeggio,
    Chord,
}

impl ViewKind {
    pub const ALL: [ViewKind; 3] = [ViewKind::Scale, ViewKind::Arpeggio, ViewKind::Chord];

    pub fn name(self) -> &'static str {
        match self {
            ViewKind::Scale => "Scale",
            ViewKind::Arpeggio => "Arpeggio",
            ViewKind::Chord => "Chord",
        }
    }
}

/// The user's current choice. `scale` is the underlying scale of an arpeggio or chord.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    pub kind: ViewKind,
    pub root: Note,
    pub scale: ScaleId,
    pub chord: String,
}

impl Selection {
    pub fn quality(&self) -> &'static ChordQuality {
        ChordQuality::by_symbol(&self.chord).unwrap_or(&super::chord::CHORDS[0])
    }

    pub fn title(&self) -> String {
        match self.kind {
            ViewKind::Scale => format!("{} {}", self.root, self.scale.name()),
            ViewKind::Arpeggio => format!("{}{} arpeggio", self.root, self.quality().suffix()),
            ViewKind::Chord => format!("{}{}", self.root, self.quality().suffix()),
        }
    }

    /// Root-independent identity, used to attach transposable forms.
    pub fn shape_key(&self) -> String {
        match self.kind {
            ViewKind::Scale => format!("scale:{:?}:{}", self.scale.family, self.scale.mode),
            ViewKind::Arpeggio => format!("arpeggio:{}", self.chord),
            ViewKind::Chord => format!("chord:{}", self.chord),
        }
    }
}

/// How a note relates to the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Root,
    /// A tone of the scale / arpeggio / chord being studied.
    Tone,
    /// A tone of the underlying scale only (drawn dimmed).
    ScaleOnly,
}

#[derive(Clone, Debug)]
pub struct FretNote {
    pub pos: Pos,
    pub midi: u8,
    pub note: Note,
    pub interval: Interval,
    pub role: Role,
}

impl FretNote {
    pub fn is_main(&self) -> bool {
        self.role != Role::ScaleOnly
    }
}

/// Everything about one selection on one tuning.
#[derive(Clone, Debug)]
pub struct FretboardModel {
    pub title: String,
    /// Every position, open string to fret 24, holding a tone or a scale tone.
    pub notes: Vec<FretNote>,
    /// The spelled tones of the selection, root first.
    pub tones: Vec<(Note, Interval)>,
    /// The spelled tones of the underlying scale.
    pub scale_tones: Vec<(Note, Interval)>,
    /// CAGED positions (empty when the tuning is not standard-based).
    pub caged: Vec<CagedPosition>,
}

impl FretboardModel {
    pub fn note_at(&self, pos: Pos) -> Option<&FretNote> {
        self.notes.iter().find(|n| n.pos == pos)
    }
}

/// A chord built on one degree of a scale.
#[derive(Clone, Debug)]
pub struct DiatonicChord {
    pub numeral: String,
    pub root: Note,
    pub quality: &'static ChordQuality,
    pub mode: ScaleId,
}

/// One note of a playback sequence.
#[derive(Clone, Copy, Debug)]
pub struct PlayStep {
    pub pos: Pos,
    pub midi: u8,
    /// Onset in seconds from the start of playback.
    pub at: f32,
}

#[derive(Default)]
pub struct ScaleEngine;

impl ScaleEngine {
    pub fn new() -> Self {
        Self
    }

    /// Spelled tones of a scale on a root.
    pub fn scale_tones(&self, root: Note, scale: ScaleId) -> Vec<(Note, Interval)> {
        scale.intervals().into_iter().map(|i| (root.transpose(i), i)).collect()
    }

    /// Spelled tones of a chord quality on a root.
    pub fn chord_tones(&self, root: Note, quality: &ChordQuality) -> Vec<(Note, Interval)> {
        quality.intervals().into_iter().map(|i| (root.transpose(i), i)).collect()
    }

    /// Build the full fretboard model for a selection.
    pub fn compute(&self, sel: &Selection, tuning: &Tuning) -> FretboardModel {
        let scale_tones = self.scale_tones(sel.root, sel.scale);
        let tones = match sel.kind {
            ViewKind::Scale => scale_tones.clone(),
            _ => self.chord_tones(sel.root, sel.quality()),
        };
        let root_pc = sel.root.pc();
        let mut notes = Vec::new();
        for string in 0..tuning.string_count() as u8 {
            for fret in 0..=FRETS {
                let midi = tuning.midi(string, fret);
                let pc = midi % 12;
                let pos = Pos::new(string, fret);
                if let Some(&(note, interval)) = tones.iter().find(|(n, _)| n.pc() == pc) {
                    let role = if pc == root_pc { Role::Root } else { Role::Tone };
                    notes.push(FretNote { pos, midi, note, interval, role });
                } else if let Some(&(note, interval)) = scale_tones.iter().find(|(n, _)| n.pc() == pc) {
                    notes.push(FretNote { pos, midi, note, interval, role: Role::ScaleOnly });
                }
            }
        }
        let caged = caged::positions(tuning, root_pc, sel.quality_for_caged());
        FretboardModel { title: sel.title(), notes, tones, scale_tones, caged }
    }

    /// The seventh chords built on each degree of a heptatonic scale (triads otherwise empty).
    pub fn diatonic_chords(&self, root: Note, scale: ScaleId) -> Vec<DiatonicChord> {
        if !scale.family.is_heptatonic() {
            return Vec::new();
        }
        let ivs = scale.intervals();
        (0..7)
            .filter_map(|d| {
                let mode = scale.mode_on_degree(d);
                let m = mode.intervals();
                let quality = ChordQuality::find(&[m[0], m[2], m[4], m[6]])?;
                Some(DiatonicChord {
                    numeral: roman(ivs[d], m[2].semitones),
                    root: root.transpose(ivs[d]),
                    quality,
                    mode,
                })
            })
            .collect()
    }

    /// Modes (of every family) that contain all tones of the chord: candidate underlying scales.
    pub fn matching_scales(&self, quality: &ChordQuality) -> Vec<ScaleId> {
        let need: Vec<u8> = quality.intervals().iter().map(|i| i.pc()).collect();
        ScaleId::all()
            .into_iter()
            .filter(|s| {
                let have: Vec<u8> = s.intervals().iter().map(|i| i.pc()).collect();
                need.iter().all(|p| have.contains(p))
            })
            .collect()
    }

    /// Positions to play when no form is active: inside a CAGED window, or
    /// otherwise the first position from the lowest root on the lowest string.
    pub fn default_positions(
        &self,
        model: &FretboardModel,
        sel: &Selection,
        tuning: &Tuning,
        caged: Option<&CagedPosition>,
    ) -> Vec<Pos> {
        if sel.kind == ViewKind::Chord
            && let Some(c) = caged.or(model.caged.first())
        {
            return c.voicing.clone();
        }
        let (lo, hi) = match caged {
            Some(c) => window_for(sel.kind, c),
            None => {
                let open = tuning.open[0] % 12;
                let r = (sel.root.pc() as i16 - open as i16).rem_euclid(12) as u8;
                (r.saturating_sub(1), r + 4)
            }
        };
        model
            .notes
            .iter()
            .filter(|n| n.is_main() && (lo..=hi).contains(&n.pos.fret))
            .map(|n| n.pos)
            .collect()
    }

    /// Turn positions into a timed sequence: a strum for chords, otherwise
    /// up and back down in pitch order at `notes_per_sec`.
    pub fn playback(&self, kind: ViewKind, positions: &[Pos], tuning: &Tuning, notes_per_sec: f32) -> Vec<PlayStep> {
        let mut notes: Vec<(Pos, u8)> =
            positions.iter().map(|&p| (p, tuning.midi(p.string, p.fret))).collect();
        notes.sort_by_key(|&(p, m)| (m, p.string));
        notes.dedup_by_key(|(_, m)| *m);
        if kind == ViewKind::Chord {
            return notes
                .iter()
                .enumerate()
                .map(|(i, &(pos, midi))| PlayStep { pos, midi, at: i as f32 * 0.04 })
                .collect();
        }
        let mut seq = notes.clone();
        seq.extend(notes.iter().rev().skip(1));
        let dt = 1.0 / notes_per_sec.max(0.5);
        seq.into_iter()
            .enumerate()
            .map(|(i, (pos, midi))| PlayStep { pos, midi, at: i as f32 * dt })
            .collect()
    }
}

/// The fret window a CAGED position covers for a given view; scales get one extra fret each side.
pub fn window_for(kind: ViewKind, c: &CagedPosition) -> (u8, u8) {
    match kind {
        ViewKind::Scale => (c.lo.saturating_sub(1), (c.hi + 1).min(FRETS)),
        _ => (c.lo, c.hi),
    }
}

impl Selection {
    /// CAGED positions are always built from a chord; for a scale we use its tonic triad/7th.
    fn quality_for_caged(&self) -> &'static ChordQuality {
        if self.kind != ViewKind::Scale {
            return self.quality();
        }
        let ivs = self.scale.intervals();
        let third = ivs.iter().find(|i| i.number == 3).map(|i| i.semitones);
        let symbol = if third == Some(3) { "m" } else { "maj" };
        ChordQuality::by_symbol(symbol).unwrap()
    }
}

/// Roman numeral of a scale degree: accidental from the degree, case from the chord's third.
fn roman(degree: Interval, third: u8) -> String {
    let label = degree.label();
    let accidental = label.trim_end_matches(char::is_numeric);
    let r = ["I", "II", "III", "IV", "V", "VI", "VII"][degree.number as usize - 1];
    let r = if third == 3 { r.to_lowercase() } else { r.to_string() };
    format!("{accidental}{r}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::caged::CagedShape;
    use crate::theory::note::Letter;
    use crate::theory::scale::ScaleFamily;
    use crate::theory::tuning::TUNINGS;

    fn sel(kind: ViewKind, root: Note, chord: &str) -> Selection {
        Selection { kind, root, scale: ScaleId::new(ScaleFamily::Major, 0), chord: chord.into() }
    }

    #[test]
    fn diatonic_chords_of_c_major() {
        let e = ScaleEngine::new();
        let chords = e.diatonic_chords(Note::new(Letter::C, 0), ScaleId::new(ScaleFamily::Major, 0));
        let names: Vec<String> =
            chords.iter().map(|c| format!("{} {}{}", c.numeral, c.root, c.quality.suffix())).collect();
        assert_eq!(
            names,
            ["I Cmaj7", "ii Dm7", "iii Em7", "IV Fmaj7", "V G7", "vi Am7", "vii Bm7b5"]
        );
    }

    #[test]
    fn diatonic_chords_of_harmonic_minor() {
        let e = ScaleEngine::new();
        let chords =
            e.diatonic_chords(Note::new(Letter::A, 0), ScaleId::new(ScaleFamily::HarmonicMinor, 0));
        let names: Vec<String> = chords.iter().map(|c| format!("{}{}", c.root, c.quality.suffix())).collect();
        assert_eq!(names, ["Am(maj7)", "Bm7b5", "Cmaj7#5", "Dm7", "E7", "Fmaj7", "G#dim7"]);
        let numerals: Vec<&str> = chords.iter().map(|c| c.numeral.as_str()).collect();
        assert_eq!(numerals, ["i", "ii", "bIII", "iv", "V", "bVI", "vii"]);
    }

    #[test]
    fn caged_c_major_open_shapes() {
        let e = ScaleEngine::new();
        let m = e.compute(&sel(ViewKind::Chord, Note::new(Letter::C, 0), "maj"), &TUNINGS[0]);
        let c = &m.caged[0];
        assert_eq!(c.shape, CagedShape::C);
        // x32010
        let frets: Vec<(u8, u8)> = c.voicing.iter().map(|p| (p.string, p.fret)).collect();
        assert_eq!(frets, [(1, 3), (2, 2), (3, 0), (4, 1), (5, 0)]);
        let shapes: Vec<CagedShape> = m.caged.iter().take(5).map(|c| c.shape).collect();
        assert_eq!(shapes, CagedShape::ALL);
        // E shape at fret 8: 8 10 10 9 8 8
        let eshape = m.caged.iter().find(|c| c.shape == CagedShape::E).unwrap();
        let frets: Vec<u8> = eshape.voicing.iter().map(|p| p.fret).collect();
        assert_eq!(frets, [8, 10, 10, 9, 8, 8]);
    }

    #[test]
    fn model_spells_tones() {
        let e = ScaleEngine::new();
        let m = e.compute(&sel(ViewKind::Arpeggio, Note::new(Letter::E, -1), "m7b5"), &TUNINGS[0]);
        let names: Vec<String> = m.tones.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(names, ["Eb", "Gb", "Bbb", "Db"]);
        assert!(m.notes.iter().all(|n| n.pos.fret <= 24));
    }

    #[test]
    fn every_chord_has_caged_voicings() {
        let e = ScaleEngine::new();
        for q in crate::theory::chord::CHORDS {
            for root in Note::selectable_roots() {
                let m = e.compute(&sel(ViewKind::Chord, root, q.symbol), &TUNINGS[0]);
                assert!(m.caged.len() >= 5, "{root}{}", q.symbol);
                assert!(m.caged.iter().all(|c| c.voicing.len() >= 3), "{root}{}", q.symbol);
            }
        }
    }
}
