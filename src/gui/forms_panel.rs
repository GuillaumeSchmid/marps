//! Forms: list, switch, record, edit and delete user-defined fingerings.

use super::app::{Recording, ScaleViewApp};
use super::theme::{ACCENT, CARD, RECORD, TEXT_DIM, caption};
use crate::forms::Form;
use crate::theory::engine::ViewKind;
use egui::{CornerRadius, Frame, Margin, RichText, Ui};
use std::collections::BTreeSet;

pub fn show(app: &mut ScaleViewApp, ui: &mut Ui) {
    Frame::new()
        .fill(CARD)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                caption(ui, "Forms");
                let sel = &app.settings.selection;
                let what = match sel.kind {
                    ViewKind::Scale => sel.scale.name().to_string(),
                    ViewKind::Arpeggio => format!("{} arpeggio", sel.quality().name),
                    ViewKind::Chord => format!("{} chord", sel.quality().name),
                };
                ui.label(RichText::new(format!("{what}, any root")).color(TEXT_DIM).small());
                ui.separator();
                if app.recording.is_some() {
                    recording_controls(app, ui);
                } else {
                    form_list(app, ui);
                }
            });
        });
}

fn form_list(app: &mut ScaleViewApp, ui: &mut Ui) {
    if ui.selectable_label(app.active_forms.is_empty(), "All notes").clicked() {
        app.clear_forms();
    }
    let key = app.form_key.clone();
    let names: Vec<String> = app.forms_here().iter().map(|f| f.name.clone()).collect();
    for (i, name) in names.iter().enumerate() {
        let (_, dropped) = ui.dnd_drop_zone::<usize, ()>(Frame::new(), |ui| {
            ui.horizontal(|ui| {
                let grip_id = egui::Id::new(("form_grip", &key, i));
                ui.dnd_drag_source(grip_id, i, |ui| {
                    ui.label(RichText::new("⠿").color(TEXT_DIM));
                });
                let text = RichText::new(name).strong();
                if ui.selectable_label(app.active_forms.contains(&i), text).clicked() {
                    app.toggle_form(i);
                }
                if ui.small_button("✏").on_hover_text("Edit this form").clicked() {
                    let form = app.forms_here()[i].clone();
                    app.recording = Some(Recording {
                        positions: form.transposed(app.settings.selection.root.pc()).into_iter().collect(),
                        name: form.name.clone(),
                        editing: Some(i),
                    });
                }
                if ui.small_button("🗑").on_hover_text("Delete this form").clicked() {
                    delete_and_reresolve(app, i);
                }
            });
        });
        if let Some(from) = dropped {
            reorder_and_reselect(app, *from, i);
        }
    }
    if names.is_empty() {
        ui.label(RichText::new("none yet — record one to save a fingering").color(TEXT_DIM).small());
    }
    ui.separator();
    if ui.button(RichText::new("⏺ New form").color(RECORD)).on_hover_text("Pick notes on the neck and name them").clicked() {
        app.recording = Some(Recording { positions: Default::default(), name: String::new(), editing: None });
        app.highlight = None;
    }
}

/// Active forms' names, so a mutation that shifts indices can re-resolve them afterward.
fn active_names(app: &ScaleViewApp) -> BTreeSet<String> {
    app.active_forms.iter().filter_map(|&i| app.forms_here().get(i)).map(|f| f.name.clone()).collect()
}

/// Indices of forms whose name is in `names`, under the current key.
fn indices_named(app: &ScaleViewApp, names: &BTreeSet<String>) -> BTreeSet<usize> {
    app.forms_here().iter().enumerate().filter(|(_, f)| names.contains(&f.name)).map(|(i, _)| i).collect()
}

/// After a drag-drop reorder, keep whatever forms were active still active
/// (their indices may have shifted) by re-resolving them by name.
fn reorder_and_reselect(app: &mut ScaleViewApp, from: usize, to: usize) {
    if from == to {
        return;
    }
    let key = app.form_key.clone();
    let names = active_names(app);
    app.forms.reorder(&key, from, to);
    app.set_active_forms(indices_named(app, &names));
}

/// Delete form `i`, then re-resolve the remaining active forms by name
/// (deleting shifts every later index down by one).
fn delete_and_reresolve(app: &mut ScaleViewApp, i: usize) {
    let key = app.form_key.clone();
    let names = active_names(app);
    app.forms.remove(&key, i);
    app.set_active_forms(indices_named(app, &names));
}

fn recording_controls(app: &mut ScaleViewApp, ui: &mut Ui) {
    let root_pc = app.settings.selection.root.pc();
    let key = app.form_key.clone();
    let rec = app.recording.as_mut().unwrap();
    ui.label(RichText::new("⏺ REC").color(RECORD).strong());
    ui.label(format!("{} notes", rec.positions.len()));
    let name = ui.add(egui::TextEdit::singleline(&mut rec.name).hint_text("Form name").desired_width(160.0));
    let ready = !rec.name.trim().is_empty() && !rec.positions.is_empty();
    let enter = name.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let save = ui.add_enabled(ready, egui::Button::new(RichText::new("Save").color(ACCENT).strong())).clicked();
    if (save || enter) && ready {
        let form = Form { name: rec.name.trim().to_string(), root_pc, positions: rec.positions.iter().copied().collect() };
        let editing = rec.editing;
        let i = app.forms.put(&key, editing, form);
        app.recording = None;
        let mut forms = app.active_forms.clone();
        forms.insert(i);
        app.set_active_forms(forms);
        return;
    }
    if ui.button("Clear").clicked() {
        rec.positions.clear();
    }
    if ui.button("Cancel").clicked() {
        app.recording = None;
    }
}
