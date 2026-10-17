//! Selection bar (above the neck) and option bar (below it).

use super::app::ScaleViewApp;
use super::theme::{self, CARD, TEXT_DIM, caption};
use crate::settings::{DEFAULT_COLORS, LabelMode, NeckWood};
use crate::theory::caged::CagedShape;
use crate::theory::chord::{CHORDS, ChordCategory};
use crate::theory::engine::ViewKind;
use crate::theory::note::{DEGREE_CLASS_LABELS, Note};
use crate::theory::scale::{ScaleFamily, ScaleId};
use crate::theory::tuning::TUNINGS;
use egui::{ComboBox, CornerRadius, Frame, Margin, RichText, Slider, Ui};

fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(CARD)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, add)
        .inner
}

pub fn selection_bar(app: &mut ScaleViewApp, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("ScaleView").strong().size(18.0).color(theme::ACCENT));
        ui.add_space(10.0);
        let sel = &mut app.settings.selection;
        for kind in ViewKind::ALL {
            let text = RichText::new(kind.name()).size(15.0).strong();
            ui.selectable_value(&mut sel.kind, kind, text);
        }
        ui.separator();
        caption(ui, "Root");
        for root in Note::selectable_roots() {
            let text = RichText::new(root.to_string()).monospace();
            if ui.selectable_label(sel.root == root, text).clicked() {
                sel.root = root;
            }
        }
    });
    ui.add_space(2.0);

    if app.settings.selection.kind != ViewKind::Scale {
        chord_picker(app, ui);
        ui.add_space(2.0);
    }
    scale_picker(app, ui);
}

fn chord_picker(app: &mut ScaleViewApp, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        caption(ui, if app.settings.selection.kind == ViewKind::Chord { "Chord" } else { "Arpeggio" });
        for cat in ChordCategory::ALL {
            ui.selectable_value(&mut app.chord_category, cat, cat.name());
        }
    });
    ui.horizontal_wrapped(|ui| {
        let sel = &mut app.settings.selection;
        for q in CHORDS.iter().filter(|q| q.category == app.chord_category) {
            let text = RichText::new(format!("{}{}", sel.root, q.suffix())).size(15.0);
            if ui.selectable_label(sel.chord == q.symbol, text).on_hover_text(q.name).clicked() {
                sel.chord = q.symbol.to_string();
            }
        }
    });
}

fn scale_picker(app: &mut ScaleViewApp, ui: &mut Ui) {
    let kind = app.settings.selection.kind;
    ui.horizontal_wrapped(|ui| {
        caption(ui, if kind == ViewKind::Scale { "Scale" } else { "Underlying scale" });
        let sel = &mut app.settings.selection;
        ComboBox::from_id_salt("family").selected_text(sel.scale.family.name()).width(220.0).show_ui(ui, |ui| {
            for f in ScaleFamily::ALL {
                if ui.selectable_label(sel.scale.family == f, f.name()).clicked() {
                    sel.scale = ScaleId::new(f, 0);
                }
            }
        });
        for (m, name) in sel.scale.family.mode_names().iter().enumerate() {
            let id = ScaleId::new(sel.scale.family, m);
            ui.selectable_value(&mut sel.scale, id, *name);
        }
    });

    if kind != ViewKind::Scale {
        // Scales (of any family) containing every chord tone.
        let fits = app.engine.matching_scales(app.settings.selection.quality());
        ui.horizontal_wrapped(|ui| {
            caption(ui, "Fits over");
            if fits.is_empty() {
                ui.label(RichText::new("no catalogued scale contains every tone").color(TEXT_DIM));
            }
            for s in fits {
                let sel = &mut app.settings.selection;
                ui.selectable_value(&mut sel.scale, s, RichText::new(s.name()).small());
            }
        });
    }

    let sel = app.settings.selection.clone();
    let chords = app.engine.diatonic_chords(sel.root, sel.scale);
    if !chords.is_empty() {
        ui.horizontal_wrapped(|ui| {
            caption(ui, "Diatonic 7ths");
            for c in chords {
                let active = sel.kind != ViewKind::Scale && c.root == sel.root && sel.chord == c.quality.symbol;
                let text = RichText::new(format!("{}  {}{}", c.numeral, c.root, c.quality.suffix()));
                let hover = format!("{} — {} {}", c.quality.name, c.root, c.mode.name());
                if ui.selectable_label(active, text).on_hover_text(hover).clicked() {
                    let s = &mut app.settings.selection;
                    s.root = c.root;
                    s.chord = c.quality.symbol.to_string();
                    s.scale = c.mode;
                    if s.kind == ViewKind::Scale {
                        s.kind = ViewKind::Arpeggio;
                    }
                    app.chord_category = c.quality.category;
                }
            }
        });
    }
}

pub fn options_bar(app: &mut ScaleViewApp, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        card(ui, |ui| {
            let now = ui.input(|i| i.time);
            let label = if app.is_playing() { "■  Stop" } else { "▶  Play" };
            let play = ui
                .add(egui::Button::new(RichText::new(label).size(16.0).strong()).min_size(egui::vec2(96.0, 30.0)))
                .on_hover_text("Play the chord / arpeggio / scale (Space)");
            if play.clicked() {
                if app.is_playing() { app.stop() } else { app.play(now) }
            }
            ui.add_enabled(
                app.settings.selection.kind != ViewKind::Chord,
                Slider::new(&mut app.settings.notes_per_sec, 1.0..=14.0).text("notes/s").max_decimals(1),
            );
        });
        card(ui, |ui| {
            caption(ui, "Show");
            let m = &mut app.settings.label_mode;
            ui.selectable_value(m, LabelMode::Dots, "Dots");
            ui.selectable_value(m, LabelMode::Degrees, "Degrees");
            ui.selectable_value(m, LabelMode::Notes, "Notes");
            ui.separator();
            ui.add_enabled(
                app.settings.selection.kind != ViewKind::Scale,
                egui::Checkbox::new(&mut app.settings.show_scale, "Underlying scale"),
            );
            ui.checkbox(&mut app.settings.colored_notes, "Coloured notes")
                .on_hover_text("Off: plain white notes on rosewood, black on maple");
        });
        card(ui, |ui| {
            caption(ui, "CAGED");
            ui.add_enabled_ui(!app.model.caged.is_empty(), |ui| {
                ui.selectable_value(&mut app.caged, None, "All");
                for s in CagedShape::ALL {
                    ui.selectable_value(&mut app.caged, Some(s), RichText::new(s.name()).strong())
                        .on_hover_text(format!("{} shape position", s.name()));
                }
            });
        });
        card(ui, |ui| {
            caption(ui, "Tuning / neck");
            ComboBox::from_id_salt("tuning").selected_text(&app.settings.tuning).show_ui(ui, |ui| {
                for t in TUNINGS {
                    ui.selectable_value(&mut app.settings.tuning, t.name.to_string(), t.name);
                }
            });
            ComboBox::from_id_salt("neck").selected_text(app.settings.neck.name()).show_ui(ui, |ui| {
                for w in NeckWood::ALL {
                    ui.selectable_value(&mut app.settings.neck, w, w.name());
                }
            });
            ui.toggle_value(&mut app.show_colors, "🎨 Colours");
        });
    });
}

pub fn color_editor(app: &mut ScaleViewApp, ui: &mut Ui) {
    card(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            caption(ui, "Degree colours");
            for (i, label) in DEGREE_CLASS_LABELS.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    ui.color_edit_button_srgb(&mut app.settings.colors[i]);
                    ui.label(RichText::new(*label).small());
                });
                ui.add_space(6.0);
            }
            if ui.button("Reset").clicked() {
                app.settings.colors = DEFAULT_COLORS;
            }
        });
    });
}
