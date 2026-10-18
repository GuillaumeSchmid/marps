//! Application state and top-level layout. Glues the ScaleEngine (data), the
//! AudioEngine (sound), persistence and the widgets together.

use super::fretboard::{Band, ChordShape, FretboardView};
use super::theme::{self, BG, PANEL};
use super::{controls, file_menu, forms_panel, header};
use crate::audio::{AudioEngine, PhraseNote};
use crate::forms::{Form, FormLibrary};
use crate::settings::Settings;
use crate::theory::Pos;
use crate::theory::caged::CagedShape;
use crate::theory::chord::ChordCategory;
use crate::theory::engine::{FretboardModel, PlayStep, ScaleEngine, Selection, ViewKind, window_for};
use crate::theory::tuning::Tuning;
use egui::{Frame, Key, Margin};
use std::collections::{BTreeSet, HashSet};

pub struct Recording {
    pub positions: BTreeSet<Pos>,
    pub name: String,
    /// Index of the form being edited, or None for a new one.
    pub editing: Option<usize>,
}

struct Playing {
    steps: Vec<PlayStep>,
    start: f64,
    chord: bool,
}

pub struct ScaleViewApp {
    pub engine: ScaleEngine,
    pub audio: AudioEngine,
    pub settings: Settings,
    saved_settings: Settings,
    pub forms: FormLibrary,
    pub file_menu: file_menu::FileMenu,
    pub model: FretboardModel,
    model_for: (Selection, String),
    /// Highlighted pitch class, and the position it was picked from.
    pub highlight: Option<(u8, Option<Pos>)>,
    pub caged: Option<CagedShape>,
    pub active_forms: BTreeSet<usize>,
    pub form_key: String,
    pub recording: Option<Recording>,
    playing: Option<Playing>,
    pub show_colors: bool,
    /// Chord category shown in the chord picker.
    pub chord_category: ChordCategory,
}

impl ScaleViewApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let settings = Settings::load();
        let engine = ScaleEngine::new();
        let tuning = Tuning::by_name(&settings.tuning);
        let model = engine.compute(&settings.selection, tuning);
        let chord_category = settings.selection.quality().category;
        let mut app = Self {
            engine,
            audio: AudioEngine::new(),
            saved_settings: settings.clone(),
            model_for: (settings.selection.clone(), settings.tuning.clone()),
            settings,
            forms: FormLibrary::load(),
            file_menu: file_menu::FileMenu::new(),
            model,
            highlight: None,
            caged: None,
            active_forms: BTreeSet::new(),
            form_key: String::new(),
            recording: None,
            playing: None,
            show_colors: false,
            chord_category,
        };
        app.sync_forms();
        app
    }

    pub fn tuning(&self) -> &'static Tuning {
        Tuning::by_name(&self.settings.tuning)
    }

    pub fn forms_here(&self) -> &[Form] {
        self.forms.forms(&self.form_key)
    }

    /// Union of every active form's positions, transposed to the current root.
    pub fn form_positions(&self) -> Option<Vec<Pos>> {
        if self.active_forms.is_empty() {
            return None;
        }
        let root_pc = self.settings.selection.root.pc();
        let forms_here = self.forms_here();
        let mut positions: BTreeSet<Pos> = BTreeSet::new();
        for &i in &self.active_forms {
            if let Some(form) = forms_here.get(i) {
                positions.extend(form.transposed(root_pc));
            }
        }
        Some(positions.into_iter().collect())
    }

    /// Flip whether `index` is one of the active (shown) forms.
    pub fn toggle_form(&mut self, index: usize) {
        let mut forms = self.active_forms.clone();
        if !forms.remove(&index) {
            forms.insert(index);
        }
        self.set_active_forms(forms);
    }

    /// Deactivate every form for the current selection.
    pub fn clear_forms(&mut self) {
        self.set_active_forms(BTreeSet::new());
    }

    /// Replace the active set outright (e.g. after a delete/reorder re-resolves
    /// indices by name) and persist it.
    pub fn set_active_forms(&mut self, forms: BTreeSet<usize>) {
        self.active_forms = forms;
        let names: BTreeSet<String> =
            self.active_forms.iter().filter_map(|&i| self.forms_here().get(i)).map(|f| f.name.clone()).collect();
        if names.is_empty() {
            self.settings.last_forms.remove(&self.form_key);
        } else {
            self.settings.last_forms.insert(self.form_key.clone(), names);
        }
    }

    /// Recompute the model and recall forms when the selection or tuning changed.
    fn refresh(&mut self) {
        let wanted = (self.settings.selection.clone(), self.settings.tuning.clone());
        if wanted != self.model_for {
            let entering_chord = wanted.0.kind == ViewKind::Chord && self.model_for.0.kind != ViewKind::Chord;
            self.model = self.engine.compute(&wanted.0, self.tuning());
            // A chord reads best one shape at a time; "All" stays one click away.
            if entering_chord && self.caged.is_none() {
                self.caged = self.model.caged.first().map(|c| c.shape);
            }
            self.model_for = wanted;
            if self.model.caged.is_empty() {
                self.caged = None;
            }
        }
        self.sync_forms();
    }

    fn sync_forms(&mut self) {
        let key = FormLibrary::key(&self.settings.tuning, &self.settings.selection.shape_key());
        if key != self.form_key {
            self.form_key = key;
            self.recording = None;
            let names = self.settings.last_forms.get(&self.form_key).cloned().unwrap_or_default();
            self.active_forms = self
                .forms_here()
                .iter()
                .enumerate()
                .filter(|(_, f)| names.contains(&f.name))
                .map(|(i, _)| i)
                .collect();
        }
    }

    pub fn play(&mut self, now: f64) {
        let sel = &self.settings.selection;
        let tuning = self.tuning();
        let positions = match (&self.recording, self.form_positions()) {
            (Some(r), _) if !r.positions.is_empty() => r.positions.iter().copied().collect(),
            (_, Some(p)) => p,
            _ => {
                let window = self.caged.and_then(|s| self.model.caged.iter().find(|c| c.shape == s));
                self.engine.default_positions(&self.model, sel, tuning, window)
            }
        };
        let steps = self.engine.playback(sel.kind, &positions, tuning, self.settings.notes_per_sec);
        self.audio.play(
            steps.iter().map(|s| PhraseNote { midi: s.midi, at: s.at, group: Some(s.pos.string) }).collect(),
        );
        self.playing = Some(Playing { steps, start: now, chord: sel.kind == ViewKind::Chord });
    }

    pub fn stop(&mut self) {
        self.audio.stop();
        self.playing = None;
    }

    pub fn is_playing(&self) -> bool {
        self.playing.is_some()
    }

    fn sounding(&mut self, now: f64) -> Vec<Pos> {
        let Some(p) = &self.playing else { return Vec::new() };
        let t = (now - p.start) as f32;
        let dur = if p.chord { 1.5 } else { 0.95 / self.settings.notes_per_sec };
        let last = p.steps.last().map_or(0.0, |s| s.at);
        if t > last + dur.max(0.4) {
            self.playing = None;
            return Vec::new();
        }
        p.steps.iter().filter(|s| t >= s.at && t < s.at + dur).map(|s| s.pos).collect()
    }

    fn on_click(&mut self, pos: Pos) {
        if let Some(rec) = &mut self.recording {
            if !rec.positions.remove(&pos) {
                rec.positions.insert(pos);
            }
            return;
        }
        let Some(midi) = self.tuning().midi(pos.string, pos.fret) else { return };
        let pc = midi % 12;
        self.highlight = match self.highlight {
            Some((h, Some(p))) if h == pc && p == pos => None,
            _ => Some((pc, Some(pos))),
        };
        self.audio.play(vec![PhraseNote { midi, at: 0.0, group: Some(pos.string) }]);
    }

    /// Notes drawn at full strength; everything else is ghosted.
    fn focus(&self) -> Option<HashSet<Pos>> {
        if self.recording.is_some() {
            return None;
        }
        if let Some(p) = self.form_positions() {
            return Some(p.into_iter().collect());
        }
        let kind = self.settings.selection.kind;
        let positions: Vec<_> = self.model.caged.iter().filter(|c| self.caged.is_none_or(|s| c.shape == s)).collect();
        match (kind, self.caged) {
            (ViewKind::Chord, _) if !positions.is_empty() => {
                Some(positions.iter().flat_map(|c| c.voicing.iter().copied()).collect())
            }
            (_, Some(_)) => Some(
                self.model
                    .notes
                    .iter()
                    .filter(|n| {
                        positions.iter().any(|c| {
                            let (lo, hi) = window_for(kind, c);
                            (lo..=hi).contains(&n.pos.fret)
                        })
                    })
                    .map(|n| n.pos)
                    .collect(),
            ),
            _ => None,
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (space, esc) = ctx.input(|i| (i.key_pressed(Key::Space), i.key_pressed(Key::Escape)));
        let now = ctx.input(|i| i.time);
        if space {
            if self.is_playing() { self.stop() } else { self.play(now) }
        }
        if esc {
            self.highlight = None;
            self.recording = None;
        }
    }
}

impl eframe::App for ScaleViewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        self.refresh();
        self.file_menu.update(&ctx, &mut self.forms);

        let bar = Frame::new().fill(PANEL).inner_margin(Margin::symmetric(16, 10));
        egui::Panel::top("selection").frame(bar).show(ui, |ui| controls::selection_bar(self, ui));
        egui::Panel::bottom("options").frame(bar).show(ui, |ui| {
            controls::options_bar(self, ui);
            ui.add_space(4.0);
            forms_panel::show(self, ui);
            if self.show_colors {
                ui.add_space(4.0);
                controls::color_editor(self, ui);
            }
        });
        self.refresh();

        let now = ctx.input(|i| i.time);
        let sounding = self.sounding(now);
        let focus = self.focus();
        let bands: Vec<Band> = self
            .model
            .caged
            .iter()
            .map(|c| Band { label: c.shape.name(), lo: c.lo, hi: c.hi, active: self.caged == Some(c.shape) })
            .collect();
        // A chord is shown as its voicings only; forms and recording show everything.
        let chord_shapes = self.settings.selection.kind == ViewKind::Chord
            && self.recording.is_none()
            && self.active_forms.is_empty();
        let shapes: Vec<ChordShape> = if chord_shapes {
            self.model
                .caged
                .iter()
                .filter(|c| self.caged.is_none_or(|s| c.shape == s))
                .map(|c| ChordShape { label: c.shape.name(), lo: c.lo, positions: c.voicing.clone() })
                .collect()
        } else {
            Vec::new()
        };

        let central = Frame::new().fill(BG).inner_margin(Margin::symmetric(18, 12));
        egui::CentralPanel::default().frame(central).show(ui, |ui| {
            header::show(self, ui);
            let clicked = FretboardView {
                model: &self.model,
                tuning: self.tuning(),
                colors: &self.settings.colors,
                label_mode: self.settings.label_mode,
                neck: self.settings.neck,
                show_scale: self.settings.show_scale,
                colored_notes: self.settings.colored_notes,
                highlight_pc: self.highlight.map(|h| h.0),
                selected: self.highlight.and_then(|h| h.1),
                focus: focus.as_ref(),
                hide_unfocused: chord_shapes,
                shapes: &shapes,
                marked: self.recording.as_ref().map(|r| &r.positions),
                bands: &bands,
                sounding: &sounding,
            }
            .show(ui);
            if let Some(pos) = clicked {
                self.on_click(pos);
            }
        });

        if self.settings != self.saved_settings {
            self.settings.save();
            self.saved_settings = self.settings.clone();
        }
    }
}
