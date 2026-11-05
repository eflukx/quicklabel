//! The look: ChopShop's fonts and its light theme.

use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};

/// Window background.
pub const INK: Color32 = Color32::from_rgb(0xf4, 0xf6, 0xf3);
/// Fields and cards.
pub const PLATE: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
/// Lines and switched-off badges.
pub const RULE: Color32 = Color32::from_rgb(0xe2, 0xe7, 0xe1);
/// Text.
pub const TEXT: Color32 = Color32::from_rgb(0x1b, 0x22, 0x1f);
/// Secondary text.
pub const MUTED: Color32 = Color32::from_rgb(0x5b, 0x67, 0x61);
/// Hints.
pub const FAINT: Color32 = Color32::from_rgb(0x98, 0xa3, 0x9d);
/// The green of switched-on badges.
pub const GREEN: Color32 = Color32::from_rgb(0x92, 0xd0, 0x50);
/// Text on green.
pub const GREEN_INK: Color32 = Color32::from_rgb(0x17, 0x30, 0x0f);
/// Good news.
pub const ACCENT: Color32 = Color32::from_rgb(0x46, 0x8a, 0x1c);
/// Bad news.
pub const ALARM: Color32 = Color32::from_rgb(0xd6, 0x45, 0x30);

/// Barlow Semi Condensed at `size`.
#[must_use]
pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Barlow Semi Condensed SemiBold at `size`.
#[must_use]
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

/// Fonts and colours.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "barlow".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../../assets/fonts/BarlowSemiCondensed-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "barlow-semibold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../../assets/fonts/BarlowSemiCondensed-SemiBold.ttf"
        ))),
    );
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut proportional = vec!["barlow".to_owned()];
    proportional.extend(fallbacks.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Proportional, proportional);
    let mut semibold = vec!["barlow-semibold".to_owned()];
    semibold.extend(fallbacks);
    fonts
        .families
        .insert(FontFamily::Name("semibold".into()), semibold);
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = INK;
        style.visuals.extreme_bg_color = PLATE;
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = GREEN.gamma_multiply(0.5);
        style.visuals.widgets.noninteractive.bg_stroke.color = RULE;
        style
            .text_styles
            .insert(egui::TextStyle::Body, regular(16.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, regular(16.0));
    });
}

/// A switch option: green when on, a plate when off; takes clicks.
pub fn badge(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let (fill, colour) = if on { (GREEN, GREEN_INK) } else { (RULE, TEXT) };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).font(semibold(14.0)).color(colour))
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}
