//! Above the neck: the selection's name, its formula as clickable tone chips,
//! and a status line (active form, recording, audio problems).

use super::app::ScaleViewApp;
use super::theme::{self, ACCENT, CARD, RECORD, TEXT, TEXT_DIM, text_on};
use crate::theory::engine::ViewKind;
use egui::{Align2, Color32, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, vec2};

pub fn show(app: &mut ScaleViewApp, ui: &mut egui::Ui) {
    let sel = app.settings.selection.clone();
    ui.horizontal(|ui| {
        ui.label(RichText::new(&app.model.title).heading().strong());
        let sub = match sel.kind {
            ViewKind::Scale => app.settings.selection.scale.family.name().to_string(),
            _ => format!("{}  ·  over {} {}", sel.quality().name, sel.root, sel.scale.name()),
        };
        ui.label(RichText::new(sub).color(TEXT_DIM));
    });
    ui.add_space(2.0);
    ui.horizontal_wrapped(|ui| {
        let tones = app.model.tones.clone();
        for (i, (note, interval)) in tones.iter().enumerate() {
            let color = theme::rgb(app.settings.colors[interval.pc() as usize]);
            let (rect, resp) = ui.allocate_exact_size(vec2(68.0, 30.0), Sense::click());
            let p = ui.painter();
            let highlighted = app.highlight.is_some_and(|h| h.0 == note.pc());
            let bg = if highlighted { Color32::from_rgb(0x3A, 0x40, 0x4D) } else { CARD };
            p.rect_filled(rect, CornerRadius::same(15), bg);
            if resp.hovered() || highlighted {
                p.rect_stroke(rect, CornerRadius::same(15), Stroke::new(1.2, ACCENT), StrokeKind::Inside);
            }
            let c = rect.left_center() + vec2(15.0, 0.0);
            if i == 0 {
                let r = egui::Rect::from_center_size(c, vec2(21.0, 21.0));
                p.rect_filled(r, CornerRadius::same(5), color);
            } else {
                p.circle_filled(c, 11.0, color);
            }
            let label = interval.label();
            let size = if label.len() >= 3 { 9.5 } else { 11.5 };
            p.text(c, Align2::CENTER_CENTER, label, FontId::proportional(size), text_on(color));
            p.text(
                rect.left_center() + vec2(33.0, 0.0),
                Align2::LEFT_CENTER,
                note.to_string(),
                FontId::proportional(15.0),
                TEXT,
            );
            if resp.on_hover_text("Highlight this note everywhere").clicked() {
                app.highlight = if highlighted { None } else { Some((note.pc(), None)) };
            }
        }
        if sel.kind != ViewKind::Scale && app.settings.show_scale {
            let extra: Vec<String> = app
                .model
                .scale_tones
                .iter()
                .filter(|(n, _)| !app.model.tones.iter().any(|(t, _)| t.pc() == n.pc()))
                .map(|(n, i)| format!("{n} ({})", i.label()))
                .collect();
            if !extra.is_empty() {
                ui.label(RichText::new(format!("  scale adds: {}", extra.join("  "))).color(TEXT_DIM));
            }
        }
    });

    ui.horizontal(|ui| {
        if let Some(rec) = &app.recording {
            ui.label(RichText::new("⏺ REC").color(RECORD).strong());
            ui.label(format!(
                "Click notes on the neck to add or remove them — {} selected. Name the form below and save.",
                rec.positions.len()
            ));
        } else if let Some(f) = app.active_form.and_then(|i| app.forms_here().get(i)) {
            ui.label(RichText::new(format!("Form: {}", f.name)).color(ACCENT).strong());
            ui.label(RichText::new("◀ ▶ arrow keys switch forms").color(TEXT_DIM).small());
        } else if let Some(shape) = app.caged {
            ui.label(RichText::new(format!("CAGED: {} shape", shape.name())).color(ACCENT));
        }
        if let Some(err) = app.audio.error() {
            ui.label(RichText::new(format!("🔇 audio unavailable: {err}")).color(TEXT_DIM).small());
        }
    });
    ui.add_space(4.0);
}
