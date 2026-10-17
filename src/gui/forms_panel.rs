//! Forms: list, switch, record, edit and delete user-defined fingerings.

use super::app::{Recording, ScaleViewApp};
use super::theme::{ACCENT, CARD, RECORD, TEXT_DIM, caption};
use crate::forms::Form;
use crate::theory::engine::ViewKind;
use egui::{CornerRadius, Frame, Margin, RichText, Ui};

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
    if ui.selectable_label(app.active_form.is_none(), "All notes").clicked() {
        app.set_active_form(None);
    }
    let names: Vec<String> = app.forms_here().iter().map(|f| f.name.clone()).collect();
    for (i, name) in names.iter().enumerate() {
        let text = RichText::new(name).strong();
        if ui.selectable_label(app.active_form == Some(i), text).clicked() {
            app.set_active_form(Some(i));
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
    if let Some(i) = app.active_form {
        let form = app.forms_here()[i].clone();
        if ui.button("✏ Edit").clicked() {
            app.recording = Some(Recording {
                positions: form.transposed(app.settings.selection.root.pc()).into_iter().collect(),
                name: form.name.clone(),
                editing: Some(i),
            });
        }
        if ui.button("🗑 Delete").clicked() {
            let key = app.form_key.clone();
            app.forms.remove(&key, i);
            app.set_active_form(None);
        }
    }
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
        app.set_active_form(Some(i));
        return;
    }
    if ui.button("Clear").clicked() {
        rec.positions.clear();
    }
    if ui.button("Cancel").clicked() {
        app.recording = None;
    }
}
