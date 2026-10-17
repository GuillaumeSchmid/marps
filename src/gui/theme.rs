//! Visual identity: dark studio theme, colours shared by every panel.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

pub const BG: Color32 = Color32::from_rgb(0x14, 0x16, 0x1B);
pub const PANEL: Color32 = Color32::from_rgb(0x1B, 0x1E, 0x25);
pub const CARD: Color32 = Color32::from_rgb(0x23, 0x27, 0x30);
pub const ACCENT: Color32 = Color32::from_rgb(0xF2, 0xB1, 0x3C);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8A, 0x91, 0x9E);
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE8, 0xEC);
pub const RECORD: Color32 = Color32::from_rgb(0xE5, 0x48, 0x4D);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = CARD;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT.linear_multiply(0.85);
    v.selection.stroke = Stroke::new(1.0, Color32::BLACK);
    v.hyperlink_color = ACCENT;
    let r = CornerRadius::same(6);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
    }
    v.widgets.inactive.weak_bg_fill = CARD;
    // Also the slider rail and checkbox background: must stand out from cards.
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x3A, 0x40, 0x4C);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x30, 0x35, 0x40);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT.linear_multiply(0.6));
    ctx.set_visuals(v);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 6.0);
        style.spacing.button_padding = egui::vec2(9.0, 4.0);
        style.text_styles.insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Button, FontId::new(14.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Small, FontId::new(11.5, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Heading, FontId::new(26.0, FontFamily::Proportional));
    });
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Black or white, whichever reads better on `bg`.
pub fn text_on(bg: Color32) -> Color32 {
    let l = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if l > 150.0 { Color32::from_rgb(0x12, 0x12, 0x12) } else { Color32::WHITE }
}

/// Small uppercase section caption.
pub fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text.to_uppercase()).small().color(TEXT_DIM).strong());
}
