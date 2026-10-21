//! The fretboard widget: paints the neck and the notes of a `FretboardModel`
//! and reports which position was clicked. Pure view — owns no state.

use super::theme::{self, ACCENT, RECORD, TEXT, TEXT_DIM, text_on};
use crate::settings::{LabelMode, NeckWood};
use crate::theory::Pos;
use crate::theory::engine::{FretNote, FretboardModel, Role};
use crate::theory::tuning::{FRETS, Tuning, pc_name};
use egui::{Align2, Color32, CornerRadius, FontId, Mesh, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use std::collections::{BTreeSet, HashSet};

/// A labelled fret window drawn above the neck (CAGED positions).
pub struct Band {
    pub label: &'static str,
    pub lo: u8,
    pub hi: u8,
    pub active: bool,
}

/// A chord voicing drawn as a connected shape, so a chord reads as a chord.
pub struct ChordShape {
    pub label: &'static str,
    pub lo: u8,
    pub positions: Vec<Pos>,
}

pub struct FretboardView<'a> {
    pub model: &'a FretboardModel,
    pub tuning: &'a Tuning,
    pub colors: &'a [[u8; 3]; 12],
    pub label_mode: LabelMode,
    pub neck: NeckWood,
    pub show_scale: bool,
    pub colored_notes: bool,
    /// Pitch class highlighted everywhere on the neck.
    pub highlight_pc: Option<u8>,
    /// The exact position the highlight was picked from.
    pub selected: Option<Pos>,
    /// When set, notes outside it are drawn as faint ghosts (or hidden).
    pub focus: Option<&'a HashSet<Pos>>,
    /// Hide out-of-focus tones instead of ghosting them.
    pub hide_unfocused: bool,
    pub shapes: &'a [ChordShape],
    /// Positions picked while recording a shape.
    pub marked: Option<&'a BTreeSet<Pos>>,
    pub bands: &'a [Band],
    /// Positions currently sounding during playback.
    pub sounding: &'a [Pos],
}

const LABEL_COL: f32 = 30.0;
const OPEN_COL: f32 = 46.0;
const BAND_ROW: f32 = 20.0;
const NUMBERS: f32 = 26.0;
const INLAYS: [u8; 10] = [3, 5, 7, 9, 12, 15, 17, 19, 21, 24];

struct Geometry {
    board: Rect,
    nut_x: f32,
    spacing: f32,
    strings: usize,
    radius: f32,
}

impl Geometry {
    /// Blend of real (logarithmic) fret spacing and even spacing, so the neck
    /// looks like a guitar but the high frets still fit a readable marker.
    fn fret_x(&self, n: u8) -> f32 {
        let n = n as f32;
        let total = FRETS as f32;
        let real = (1.0 - 2f32.powf(-n / 12.0)) / (1.0 - 2f32.powf(-total / 12.0));
        let even = n / total;
        self.nut_x + (0.5 * real + 0.5 * even) * (self.board.right() - self.nut_x)
    }

    fn note_x(&self, fret: u8) -> f32 {
        if fret == 0 {
            self.nut_x - OPEN_COL * 0.5
        } else {
            0.5 * (self.fret_x(fret - 1) + self.fret_x(fret))
        }
    }

    fn string_y(&self, string: u8) -> f32 {
        self.board.top() + self.spacing * (self.strings as f32 - 1.0 - string as f32 + 0.5)
    }

    fn center(&self, p: Pos) -> Pos2 {
        pos2(self.note_x(p.fret), self.string_y(p.string))
    }

    fn hit(&self, pt: Pos2) -> Option<Pos> {
        let row = ((pt.y - self.board.top()) / self.spacing).floor();
        if row < 0.0 || row >= self.strings as f32 {
            return None;
        }
        let string = (self.strings as f32 - 1.0 - row) as u8;
        if pt.x < self.nut_x {
            return (pt.x >= self.nut_x - OPEN_COL).then_some(Pos::new(string, 0));
        }
        (1..=FRETS).find(|&f| pt.x <= self.fret_x(f)).map(|f| Pos::new(string, f))
    }

    fn window_x(&self, lo: u8, hi: u8) -> (f32, f32) {
        let left = if lo == 0 { self.nut_x - OPEN_COL } else { self.fret_x(lo - 1) };
        (left, self.fret_x(hi))
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let ch = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgb(ch(a.r(), b.r()), ch(a.g(), b.g()), ch(a.b(), b.b()))
}

fn shape_color(label: &str) -> Color32 {
    match label {
        "C" => Color32::from_rgb(0x4F, 0xC3, 0xF7),
        "A" => Color32::from_rgb(0x81, 0xC7, 0x84),
        "G" => Color32::from_rgb(0xFF, 0xB7, 0x4D),
        "E" => Color32::from_rgb(0xF0, 0x62, 0x92),
        _ => Color32::from_rgb(0xBA, 0x68, 0xC8),
    }
}

impl FretboardView<'_> {
    pub fn show(self, ui: &mut Ui) -> Option<Pos> {
        let strings = self.tuning.string_count();
        let avail = ui.available_size();
        let bands_h = 2.0 * BAND_ROW + 6.0;
        let spacing = ((avail.y - bands_h - NUMBERS - 10.0) / strings as f32).clamp(24.0, 46.0);
        let height = bands_h + spacing * strings as f32 + NUMBERS;
        let (rect, response) = ui.allocate_exact_size(vec2(avail.x, height.max(avail.y)), Sense::click());
        let top = rect.center().y - height * 0.5 + bands_h;
        let board = Rect::from_min_max(
            pos2(rect.left() + LABEL_COL + OPEN_COL, top),
            pos2(rect.right() - 14.0, top + spacing * strings as f32),
        );
        let mut g = Geometry { board, nut_x: board.left(), spacing, strings, radius: 0.0 };
        let narrowest = g.fret_x(FRETS) - g.fret_x(FRETS - 1);
        g.radius = (spacing * 0.42).min(narrowest * 0.44).max(7.0);

        let painter = ui.painter_at(rect);
        self.paint_bands(&painter, &g);
        paint_neck(&painter, &g, self.tuning, &NeckLook::of(self.neck));
        self.paint_windows(&painter, &g);
        self.paint_shapes(&painter, &g);

        let hovered = response.hover_pos().and_then(|p| g.hit(p));
        let time = ui.input(|i| i.time);

        // Ghosts and dimmed scale tones first, so full markers are drawn on top.
        let mut layers: [Vec<(&FretNote, Look)>; 2] = [Vec::new(), Vec::new()];
        for n in &self.model.notes {
            if let Some(look) = self.look(n) {
                layers[(look.alpha > 0.5) as usize].push((n, look));
            }
        }
        for (n, look) in layers.iter().flatten() {
            self.paint_note(&painter, &g, n, *look, time);
        }
        if let Some(marked) = self.marked {
            for &p in marked {
                let c = g.center(p);
                painter.circle_stroke(c, g.radius + 3.5, Stroke::new(3.0, RECORD));
            }
        }

        if let Some(p) = hovered {
            let c = g.center(p);
            painter.circle_stroke(c, g.radius + 2.0, Stroke::new(1.5, Color32::from_white_alpha(140)));
            if let Some(n) = self.model.note_at(p) {
                paint_tooltip(&painter, rect, c + vec2(0.0, -g.radius - 10.0), n);
            }
        }
        if !self.sounding.is_empty() {
            ui.ctx().request_repaint();
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        response.clicked().then_some(hovered).flatten()
    }

    fn look(&self, n: &FretNote) -> Option<Look> {
        let recording = self.marked.is_some();
        let in_focus = self.focus.is_none_or(|f| f.contains(&n.pos));
        let scale_only = n.role == Role::ScaleOnly;
        if scale_only && !self.show_scale && !recording {
            return None;
        }
        if !in_focus && !scale_only && self.hide_unfocused && !recording {
            return None;
        }
        let (alpha, size) = match (scale_only, in_focus) {
            (false, true) => (1.0, 1.0),
            (true, true) => (0.32, 0.74),
            (false, false) => (0.2, 0.8),
            (true, false) => (0.12, 0.66),
        };
        Some(Look { alpha, size })
    }

    fn paint_note(&self, painter: &egui::Painter, g: &Geometry, n: &FretNote, look: Look, time: f64) {
        let c = g.center(n.pos);
        let r = g.radius * look.size;
        let base = if self.colored_notes {
            theme::rgb(self.colors[n.interval.pc() as usize])
        } else {
            NeckLook::of(self.neck).marker
        };
        let highlighted = self.highlight_pc == Some(n.midi % 12);
        let sounding = self.sounding.contains(&n.pos);
        let alpha = if highlighted || sounding { look.alpha.max(0.85) } else { look.alpha };
        let pulse = 0.5 + 0.5 * (time * 12.0).sin() as f32;
        let fill = if sounding { lerp_color(base, ACCENT, pulse).gamma_multiply(alpha) } else { base.gamma_multiply(alpha) };

        if highlighted {
            painter.circle_filled(c, r + 7.0, Color32::from_white_alpha(28));
            painter.circle_filled(c, r + 4.0, Color32::from_white_alpha(45));
        }
        if sounding {
            painter.circle_filled(c, r + 6.0 + 3.0 * pulse, ACCENT.gamma_multiply(0.35));
        }

        let shadow = Color32::from_black_alpha((110.0 * alpha) as u8);
        let outline = Stroke::new(1.4, Color32::from_black_alpha((160.0 * alpha) as u8));
        if n.role == Role::Root {
            let s = r * 0.93;
            let rr = CornerRadius::same((s * 0.32) as u8);
            let rect = Rect::from_center_size(c, vec2(2.0 * s, 2.0 * s));
            painter.rect_filled(rect.translate(vec2(0.0, 1.8)), rr, shadow);
            painter.rect_filled(rect, rr, fill);
            let gloss = Rect::from_min_max(rect.min + vec2(3.0, 2.5), pos2(rect.max.x - 3.0, c.y - 1.0));
            painter.rect_filled(gloss, rr, Color32::from_white_alpha((34.0 * alpha) as u8));
            painter.rect_stroke(rect, rr, outline, StrokeKind::Outside);
        } else {
            painter.circle_filled(c + vec2(0.0, 1.8), r, shadow);
            painter.circle_filled(c, r, fill);
            painter.circle_filled(c - vec2(0.0, r * 0.35), r * 0.55, Color32::from_white_alpha((26.0 * alpha) as u8));
            painter.circle_stroke(c, r, outline);
        }

        if highlighted {
            let ring = Stroke::new(2.4, Color32::WHITE);
            if n.role == Role::Root {
                let rect = Rect::from_center_size(c, vec2(2.0 * r + 6.0, 2.0 * r + 6.0));
                painter.rect_stroke(rect, CornerRadius::same((r * 0.4) as u8), ring, StrokeKind::Outside);
            } else {
                painter.circle_stroke(c, r + 3.0, ring);
            }
        }
        if self.selected == Some(n.pos) {
            painter.circle_stroke(c, r + 7.5, Stroke::new(2.0, ACCENT));
        }

        let label = match self.label_mode {
            LabelMode::Dots => return,
            LabelMode::Degrees => n.interval.label(),
            LabelMode::Notes => n.note.to_string(),
        };
        let size = r * if label.len() >= 3 { 0.82 } else { 1.0 };
        let text = text_on(base).gamma_multiply(alpha.max(0.35));
        painter.text(c + vec2(0.0, 0.5), Align2::CENTER_CENTER, label, FontId::proportional(size), text);
    }

    fn paint_bands(&self, painter: &egui::Painter, g: &Geometry) {
        let any_active = self.bands.iter().any(|b| b.active);
        for (i, b) in self.bands.iter().enumerate() {
            let (x0, x1) = g.window_x(b.lo, b.hi);
            let row = (i % 2) as f32;
            let y = g.board.top() - 6.0 - BAND_ROW * (2.0 - row);
            let r = Rect::from_min_max(pos2(x0 + 1.0, y + 2.0), pos2(x1 - 1.0, y + BAND_ROW - 2.0));
            let col = shape_color(b.label);
            let strong = b.active || !any_active;
            painter.rect_filled(r, CornerRadius::same(5), col.gamma_multiply(if strong { 0.55 } else { 0.14 }));
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                format!("{} shape", b.label),
                FontId::proportional(11.5),
                if strong { text_on(col) } else { TEXT_DIM },
            );
        }
    }

    /// Join each voicing's notes with a ribbon in its CAGED colour and mark
    /// unplayed strings with a cross at the window's edge.
    fn paint_shapes(&self, painter: &egui::Painter, g: &Geometry) {
        for shape in self.shapes {
            let col = shape_color(shape.label);
            let mut pts: Vec<Pos> = shape.positions.clone();
            pts.sort_by_key(|p| p.string);
            let line: Vec<Pos2> = pts.iter().map(|&p| g.center(p)).collect();
            painter.add(egui::Shape::line(line, Stroke::new(g.radius * 0.8, col.gamma_multiply(0.45))));
            let x = g.window_x(shape.lo, shape.lo).0 + 7.0;
            for s in (0..g.strings as u8).filter(|s| !pts.iter().any(|p| p.string == *s)) {
                let c = pos2(x, g.string_y(s));
                let d = 5.5;
                let stroke = Stroke::new(3.0, col.gamma_multiply(0.9));
                painter.circle_filled(c, d + 3.0, Color32::from_black_alpha(70));
                painter.line_segment([c + vec2(-d, -d), c + vec2(d, d)], stroke);
                painter.line_segment([c + vec2(-d, d), c + vec2(d, -d)], stroke);
            }
        }
    }

    /// Light the active windows on the neck itself (drawn over the wood, under the notes).
    fn paint_windows(&self, painter: &egui::Painter, g: &Geometry) {
        for b in self.bands.iter().filter(|b| b.active) {
            let (x0, x1) = g.window_x(b.lo, b.hi);
            let col = shape_color(b.label);
            let w = Rect::from_min_max(pos2(x0, g.board.top() - 3.0), pos2(x1, g.board.bottom() + 3.0));
            painter.rect_filled(w, CornerRadius::same(4), col.gamma_multiply(0.13));
            painter.rect_stroke(w, CornerRadius::same(4), Stroke::new(2.0, col.gamma_multiply(0.85)), StrokeKind::Inside);
        }
    }
}

#[derive(Clone, Copy)]
struct Look {
    alpha: f32,
    size: f32,
}

/// Colours of the fingerboard materials for one wood.
struct NeckLook {
    /// Board colour at the edges and in the middle (slightly lighter there).
    edge: Color32,
    mid: Color32,
    grain: [u8; 3],
    grain_alpha: u8,
    inlay: Color32,
    nut: Color32,
    /// Note colour when degree colours are switched off.
    marker: Color32,
    plain: Color32,
    wound: Color32,
}

impl NeckLook {
    fn of(wood: NeckWood) -> Self {
        match wood {
            NeckWood::Rosewood => NeckLook {
                edge: Color32::from_rgb(0x3A, 0x24, 0x17),
                mid: Color32::from_rgb(0x4A, 0x2F, 0x1F),
                grain: [0, 0, 0],
                grain_alpha: 10,
                inlay: Color32::from_rgba_unmultiplied(0xE8, 0xE2, 0xD0, 170),
                nut: Color32::from_rgb(0xEE, 0xE6, 0xD2),
                marker: Color32::from_rgb(0xF4, 0xF4, 0xF2),
                plain: Color32::from_rgb(0xDC, 0xDF, 0xE3),
                wound: Color32::from_rgb(0xC9, 0xA8, 0x72),
            },
            // Pale, lacquered maple with black dots and a black nut; strings
            // darker so they still read against the light wood.
            NeckWood::Maple => NeckLook {
                edge: Color32::from_rgb(0xC8, 0x9B, 0x5E),
                mid: Color32::from_rgb(0xE2, 0xBC, 0x82),
                grain: [0x8A, 0x5A, 0x2B],
                grain_alpha: 22,
                inlay: Color32::from_rgb(0x1E, 0x1A, 0x16),
                nut: Color32::from_rgb(0x22, 0x20, 0x1E),
                marker: Color32::from_rgb(0x14, 0x14, 0x14),
                plain: Color32::from_rgb(0x8C, 0x92, 0x99),
                wound: Color32::from_rgb(0x9A, 0x74, 0x3E),
            },
        }
    }
}

fn paint_neck(painter: &egui::Painter, g: &Geometry, tuning: &Tuning, look: &NeckLook) {
    let b = g.board;
    let (top, mid) = (look.edge, look.mid);
    let mut mesh = Mesh::default();
    for (i, (y, c)) in [(b.top(), top), (b.center().y, mid), (b.bottom(), top)].into_iter().enumerate() {
        mesh.colored_vertex(pos2(b.left(), y), c);
        mesh.colored_vertex(pos2(b.right(), y), c);
        if i > 0 {
            let k = 2 * i as u32;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k, k + 1);
        }
    }
    painter.add(mesh);
    // Wood grain: deterministic thin streaks.
    let mut seed: u32 = 0x1234_5678;
    for _ in 0..70 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let y = b.top() + (seed >> 8) as f32 / (1u32 << 24) as f32 * b.height();
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let x0 = b.left() + (seed >> 8) as f32 / (1u32 << 24) as f32 * b.width() * 0.8;
        let len = 60.0 + (seed % 400) as f32;
        let a = look.grain_alpha + (seed % 14) as u8;
        let [r, gr, bl] = look.grain;
        painter.line_segment(
            [pos2(x0, y), pos2((x0 + len).min(b.right()), y + ((seed % 5) as f32 - 2.0) * 0.4)],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(r, gr, bl, a)),
        );
    }
    // Binding.
    for y in [b.top(), b.bottom()] {
        painter.line_segment([pos2(b.left(), y), pos2(b.right(), y)], Stroke::new(2.0, Color32::from_rgb(0xD8, 0xCF, 0xB8)));
    }
    // Inlays.
    let pearl = look.inlay;
    // Always between two strings, never under a note: single dots in the middle
    // gap, octave doubles about a third of the way in from each edge.
    let gap_y = |k: usize| b.top() + g.spacing * k as f32;
    let n = g.strings;
    for f in INLAYS {
        let x = g.note_x(f);
        let r = (g.spacing * 0.2).min(9.0);
        if f % 12 == 0 {
            let k = (n as f32 / 3.0).round() as usize;
            painter.circle_filled(pos2(x, gap_y(k)), r, pearl);
            painter.circle_filled(pos2(x, gap_y(n - k)), r, pearl);
        } else {
            painter.circle_filled(pos2(x, gap_y(n / 2)), r, pearl);
        }
    }
    // Frets: nickel with a highlight.
    for f in 1..=FRETS {
        let x = g.fret_x(f);
        painter.line_segment([pos2(x, b.top()), pos2(x, b.bottom())], Stroke::new(3.2, Color32::from_rgb(0x8E, 0x93, 0x99)));
        painter.line_segment(
            [pos2(x - 0.6, b.top()), pos2(x - 0.6, b.bottom())],
            Stroke::new(1.0, Color32::from_rgb(0xE4, 0xE7, 0xEA)),
        );
    }
    // Nut (bone).
    let nut = Rect::from_min_max(pos2(g.nut_x - 7.0, b.top() - 1.0), pos2(g.nut_x, b.bottom() + 1.0));
    painter.rect_filled(nut, CornerRadius::same(2), look.nut);
    // Strings: wound strings bronze, plain strings steel; thicker toward the bass.
    for s in 0..n as u8 {
        let y = g.string_y(s);
        let t = s as f32 / (n as f32 - 1.0);
        let width = 3.4 - 2.3 * t;
        let wound = (s as usize) < n - 2;
        let col = if wound { look.wound } else { look.plain };
        let x0 = g.nut_x - OPEN_COL;
        painter.line_segment([pos2(x0, y + 1.6), pos2(b.right(), y + 1.6)], Stroke::new(width, Color32::from_black_alpha(90)));
        painter.line_segment([pos2(x0, y), pos2(b.right(), y)], Stroke::new(width, col));
        // String name.
        painter.text(
            pos2(b.left() - OPEN_COL - LABEL_COL * 0.5, y),
            Align2::CENTER_CENTER,
            pc_name(tuning.open[s as usize]),
            FontId::proportional(13.0),
            TEXT_DIM,
        );
    }
    // Fret numbers.
    for f in 0..=FRETS {
        let inlay = INLAYS.contains(&f);
        painter.text(
            pos2(g.note_x(f), b.bottom() + NUMBERS * 0.55),
            Align2::CENTER_CENTER,
            f.to_string(),
            FontId::proportional(if inlay { 13.0 } else { 11.0 }),
            if inlay { TEXT } else { TEXT_DIM },
        );
    }
}

fn paint_tooltip(painter: &egui::Painter, clip: Rect, anchor: Pos2, n: &FretNote) {
    let octave = n.midi as i32 / 12 - 1;
    let text = format!("{}{}  ·  {}", n.note, octave, n.interval.label());
    let galley = painter.layout_no_wrap(text, FontId::proportional(13.0), TEXT);
    let size = galley.size() + vec2(14.0, 8.0);
    let mut r = Rect::from_center_size(anchor - vec2(0.0, size.y * 0.5), size);
    r = r.translate(vec2((clip.left() - r.left()).max(0.0) + (clip.right() - r.right()).min(0.0), (clip.top() - r.top()).max(0.0)));
    painter.rect_filled(r.translate(vec2(0.0, 2.0)), CornerRadius::same(6), Color32::from_black_alpha(120));
    painter.rect_filled(r, CornerRadius::same(6), Color32::from_rgb(0x2B, 0x30, 0x3A));
    painter.galley(r.min + vec2(7.0, 4.0), galley, TEXT);
}
