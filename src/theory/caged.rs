//! The CAGED system: five fret windows per root, each with a playable voicing.

use super::chord::ChordQuality;
use super::note::Interval;
use super::tuning::{FRETS, Tuning};
use super::Pos;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CagedShape {
    C,
    A,
    G,
    E,
    D,
}

impl CagedShape {
    pub const ALL: [CagedShape; 5] =
        [CagedShape::C, CagedShape::A, CagedShape::G, CagedShape::E, CagedShape::D];

    pub fn name(self) -> &'static str {
        match self {
            CagedShape::C => "C",
            CagedShape::A => "A",
            CagedShape::G => "G",
            CagedShape::E => "E",
            CagedShape::D => "D",
        }
    }

    /// (string carrying the root, relative to the low E; window start/end relative to the root fret)
    fn geometry(self) -> (usize, i16, i16) {
        match self {
            CagedShape::C => (1, -3, 0),
            CagedShape::A => (1, 0, 3),
            CagedShape::G => (0, -3, 0),
            CagedShape::E => (0, 0, 3),
            CagedShape::D => (2, 0, 3),
        }
    }
}

/// One CAGED position on the neck.
#[derive(Clone, Debug)]
pub struct CagedPosition {
    pub shape: CagedShape,
    pub lo: u8,
    pub hi: u8,
    /// A playable chord voicing inside the window, lowest string first.
    pub voicing: Vec<Pos>,
}

/// Every CAGED position of `quality` on `root_pc` that fits on the neck, ordered by fret.
pub fn positions(tuning: &Tuning, root_pc: u8, quality: &ChordQuality) -> Vec<CagedPosition> {
    let Some(low) = tuning.caged_low_string() else { return Vec::new() };
    let tones = quality.intervals();
    let mut out = Vec::new();
    for shape in CagedShape::ALL {
        let (rel_string, lo_off, hi_off) = shape.geometry();
        let root_string = low + rel_string;
        let open_pc = tuning.open[root_string] % 12;
        let base = (root_pc as i16 - open_pc as i16).rem_euclid(12);
        for octave in 0..3 {
            let root_fret = base + 12 * octave;
            let (lo, hi) = (root_fret + lo_off, root_fret + hi_off);
            if lo < 0 || hi > FRETS as i16 {
                continue;
            }
            let voicing =
                best_voicing(tuning, root_string, root_fret as u8, lo as u8, hi as u8, root_pc, &tones, quality);
            out.push(CagedPosition { shape, lo: lo as u8, hi: hi as u8, voicing });
        }
    }
    out.sort_by_key(|p| p.lo);
    out
}

/// Exhaustive search for the best-sounding voicing inside a window: root in
/// the bass, every essential tone present, few muted strings, small stretch.
#[allow(clippy::too_many_arguments)]
fn best_voicing(
    tuning: &Tuning,
    root_string: usize,
    root_fret: u8,
    lo: u8,
    hi: u8,
    root_pc: u8,
    tones: &[Interval],
    quality: &ChordQuality,
) -> Vec<Pos> {
    let tone_of = |string: usize, fret: u8| -> Option<usize> {
        let pc = (tuning.midi(string as u8, fret)? as i16 - root_pc as i16).rem_euclid(12) as u8;
        tones.iter().position(|t| t.pc() == pc)
    };
    // Candidate frets for each string above the root string (None = muted).
    let upper: Vec<Vec<Option<u8>>> = (root_string + 1..tuning.string_count())
        .map(|s| {
            let mut c: Vec<Option<u8>> =
                (lo..=hi).filter(|&f| tone_of(s, f).is_some()).map(Some).collect();
            c.push(None);
            c
        })
        .collect();

    let mut best: Option<(i32, Vec<Option<u8>>)> = None;
    let mut choice = vec![None; upper.len()];
    search(&upper, 0, &mut choice, &mut |frets| {
        let score = score_voicing(root_string, root_fret, frets, tones, quality, &tone_of);
        if best.as_ref().is_none_or(|(b, _)| score > *b) {
            best = Some((score, frets.to_vec()));
        }
    });

    let mut voicing = vec![Pos::new(root_string as u8, root_fret)];
    if let Some((_, frets)) = best {
        for (i, f) in frets.into_iter().enumerate() {
            if let Some(f) = f {
                voicing.push(Pos::new((root_string + 1 + i) as u8, f));
            }
        }
    }
    voicing
}

fn search(
    cands: &[Vec<Option<u8>>],
    i: usize,
    choice: &mut Vec<Option<u8>>,
    visit: &mut dyn FnMut(&[Option<u8>]),
) {
    if i == cands.len() {
        visit(choice);
        return;
    }
    for &c in &cands[i] {
        choice[i] = c;
        search(cands, i + 1, choice, visit);
    }
}

fn score_voicing(
    root_string: usize,
    root_fret: u8,
    frets: &[Option<u8>],
    tones: &[Interval],
    quality: &ChordQuality,
    tone_of: &dyn Fn(usize, u8) -> Option<usize>,
) -> i32 {
    let mut present = vec![0u8; tones.len()];
    present[0] = 1;
    let mut score = 0;
    let mut fretted = vec![root_fret];
    for (i, f) in frets.iter().enumerate() {
        if let Some(f) = *f {
            let t = tone_of(root_string + 1 + i, f).unwrap();
            present[t] += 1;
            score += 3;
            if f > 0 {
                fretted.push(f);
            }
        }
    }
    // Muted strings: inner ones are awkward to damp, outer top ones just thin the chord.
    let last_sounding = frets.iter().rposition(|f| f.is_some());
    for (i, f) in frets.iter().enumerate() {
        if f.is_none() {
            score -= if last_sounding.is_some_and(|l| i < l) { 25 } else { 6 };
        }
    }
    for (t, &n) in tones.iter().zip(&present) {
        if n == 0 {
            score -= if quality.is_optional(*t) { 8 } else { 100 };
        } else if n > 1 {
            score -= 2 * (n as i32 - 1);
        }
    }
    let fretted: Vec<u8> = fretted.into_iter().filter(|&f| f > 0).collect();
    if let (Some(min), Some(max)) = (fretted.iter().min(), fretted.iter().max()) {
        score -= 3 * (max - min) as i32;
    }
    // More than four distinct fretted positions on different frets is hard to finger.
    if fretted.len() > 4 {
        let mut distinct = fretted.clone();
        distinct.sort();
        distinct.dedup();
        if distinct.len() > 3 {
            score -= 10;
        }
    }
    score
}
