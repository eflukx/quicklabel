//! The look: iotta's green and Barlow, light or dark as Windows is set.
//!
//! Working principle: two palettes (ChopShop's Fresh and Graphite). egui gets a style for each
//! theme and follows the system; the drawing code asks [`palette`] for the colours of the theme
//! shown. Plates on a soft background, no borders but a hairline, the iotta green for what acts.

use std::sync::{Arc, LazyLock};

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Response, Sense, Stroke,
    Ui, Vec2,
};

/// The colours of one theme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Window background.
    pub ink: Color32,
    /// Fields and cards.
    pub plate: Color32,
    /// The tray the label lies on.
    pub tray: Color32,
    /// Hairlines, switched-off chips.
    pub rule: Color32,
    /// Text.
    pub text: Color32,
    /// Secondary text.
    pub muted: Color32,
    /// Hints.
    pub faint: Color32,
    /// iotta green, as a fill.
    pub green: Color32,
    /// Green while pressed or hovered.
    pub green_pressed: Color32,
    /// Text on green.
    pub green_ink: Color32,
    /// Green as text on the background.
    pub accent: Color32,
    /// Waiting (checking), as a dot.
    pub echo: Color32,
    /// Bad news.
    pub alarm: Color32,
    /// Whether the palette is dark.
    pub dark: bool,
}

/// Light: ChopShop's Fresh.
pub const LIGHT: Palette = Palette {
    ink: Color32::from_rgb(0xf4, 0xf6, 0xf3),
    plate: Color32::from_rgb(0xff, 0xff, 0xff),
    tray: Color32::from_rgb(0xe9, 0xed, 0xe7),
    rule: Color32::from_rgb(0xe2, 0xe7, 0xe1),
    text: Color32::from_rgb(0x1b, 0x22, 0x1f),
    muted: Color32::from_rgb(0x5b, 0x67, 0x61),
    faint: Color32::from_rgb(0x98, 0xa3, 0x9d),
    green: Color32::from_rgb(0x92, 0xd0, 0x50),
    green_pressed: Color32::from_rgb(0x7f, 0xbc, 0x3e),
    green_ink: Color32::from_rgb(0x17, 0x30, 0x0f),
    accent: Color32::from_rgb(0x46, 0x8a, 0x1c),
    echo: Color32::from_rgb(0xe0, 0xa1, 0x1b),
    alarm: Color32::from_rgb(0xd6, 0x45, 0x30),
    dark: false,
};

/// Dark: ChopShop's Graphite.
pub const DARK: Palette = Palette {
    ink: Color32::from_rgb(0x15, 0x17, 0x19),
    plate: Color32::from_rgb(0x20, 0x23, 0x26),
    tray: Color32::from_rgb(0x0e, 0x10, 0x11),
    rule: Color32::from_rgb(0x34, 0x38, 0x3c),
    text: Color32::from_rgb(0xf1, 0xf3, 0xf1),
    muted: Color32::from_rgb(0xa3, 0xab, 0xa7),
    faint: Color32::from_rgb(0x6e, 0x76, 0x72),
    green: Color32::from_rgb(0x92, 0xd0, 0x50),
    green_pressed: Color32::from_rgb(0xa6, 0xdc, 0x6c),
    green_ink: Color32::from_rgb(0x17, 0x30, 0x0f),
    accent: Color32::from_rgb(0x9f, 0xd8, 0x62),
    echo: Color32::from_rgb(0xf5, 0xc4, 0x51),
    alarm: Color32::from_rgb(0xff, 0x7a, 0x66),
    dark: true,
};

/// The palette of the theme shown.
#[must_use]
pub fn palette(ctx: &egui::Context) -> &'static Palette {
    if ctx.theme() == egui::Theme::Dark {
        &DARK
    } else {
        &LIGHT
    }
}

/// Barlow Semi Condensed at `size`.
#[must_use]
pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Barlow Semi Condensed Medium at `size`.
#[must_use]
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}

/// Barlow Semi Condensed SemiBold at `size`.
#[must_use]
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

/// Fonts, and a style per theme.
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let faces: [(&str, &'static [u8]); 3] = [
        (
            "barlow",
            include_bytes!("../../../assets/fonts/BarlowSemiCondensed-Regular.ttf"),
        ),
        (
            "medium",
            include_bytes!("../../../assets/fonts/BarlowSemiCondensed-Medium.ttf"),
        ),
        (
            "semibold",
            include_bytes!("../../../assets/fonts/BarlowSemiCondensed-SemiBold.ttf"),
        ),
    ];
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (name, bytes) in faces {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
        let family = if name == "barlow" {
            FontFamily::Proportional
        } else {
            FontFamily::Name(name.into())
        };
        let mut list = vec![name.to_owned()];
        list.extend(fallbacks.iter().cloned());
        fonts.families.insert(family, list);
    }
    ctx.set_fonts(fonts);

    for (theme, palette) in [(egui::Theme::Light, &LIGHT), (egui::Theme::Dark, &DARK)] {
        ctx.style_mut_of(theme, |style| apply(style, palette));
    }
    ctx.set_theme(egui::ThemePreference::System);
}

fn apply(style: &mut egui::Style, c: &Palette) {
    let mut visuals = if c.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = c.ink;
    visuals.window_fill = c.plate;
    visuals.extreme_bg_color = c.plate;
    visuals.override_text_color = Some(c.text);
    visuals.hyperlink_color = c.accent;
    visuals.selection.bg_fill = c.green.gamma_multiply(0.45);
    visuals.selection.stroke = Stroke::new(1.5, c.accent);
    visuals.text_cursor.stroke = Stroke::new(2.0, c.accent);

    let radius = CornerRadius::same(6);
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, c.rule);
    widgets.noninteractive.fg_stroke.color = c.muted;
    for (state, fill) in [
        (&mut widgets.inactive, c.rule),
        (&mut widgets.hovered, c.rule.gamma_multiply(0.85)),
        (&mut widgets.active, c.green_pressed),
        (&mut widgets.open, c.rule),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::NONE;
        state.corner_radius = radius;
        state.fg_stroke.color = c.text;
        state.expansion = 0.0;
    }
    widgets.hovered.bg_stroke = Stroke::new(1.0, c.green);
    style.visuals = visuals;

    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style
        .text_styles
        .insert(egui::TextStyle::Body, regular(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, medium(15.0));
}

/// A chip: green when on, a plate when off, with a status dot when given; takes clicks.
pub fn chip(ui: &mut Ui, text: &str, on: bool, dot: Option<Color32>) -> Response {
    chip_with(ui, text, on, false, |ui, ink, fill| {
        status_dot(ui, on, dot, ink, fill)
    })
}

/// A chip that opens a menu: as [`chip`], with a small triangle after the text.
pub fn menu_chip(ui: &mut Ui, text: &str, on: bool, dot: Option<Color32>) -> Response {
    chip_with(ui, text, on, true, |ui, ink, fill| {
        status_dot(ui, on, dot, ink, fill)
    })
}

/// A chip's status dot, ringed in the chip's ink (on) or fill (off).
fn status_dot(ui: &mut Ui, on: bool, dot: Option<Color32>, ink: Color32, fill: Color32) {
    if let Some(dot) = dot {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        let ring = if on { ink } else { fill };
        ui.painter()
            .circle(rect.center(), 4.0, dot, Stroke::new(1.0, ring));
    }
}

/// The chip of the text's direction, with a small label and an upright A in front (as
/// Brother's own software shows it): wide for lengthwise, tall for sideways. A die-cut label is
/// a closed outline with round corners; continuous tape a strip whose edges run on, with dashed
/// cuts where the label ends.
pub fn direction_chip(ui: &mut Ui, text: &str, sideways: bool, continuous: bool) -> Response {
    chip_with(ui, text, true, false, |ui, ink, _| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 18.0), Sense::hover());
        let painter = ui.painter();
        let stroke = Stroke::new(1.3, ink);
        let size = match (sideways, continuous) {
            (true, true) => egui::vec2(11.0, 14.0),
            (false, true) => egui::vec2(16.0, 11.0),
            // A die-cut label's own corners need a little more room around the A.
            (true, false) => egui::vec2(13.0, 17.0),
            (false, false) => egui::vec2(19.0, 13.0),
        };
        let label = egui::Rect::from_center_size(rect.center(), size);

        if continuous {
            // The tape runs along x when lengthwise, along y when sideways (as the preview).
            let cut = Stroke::new(1.0, ink.gamma_multiply(0.8));
            let dashed = |from: egui::Pos2, to: egui::Pos2| {
                painter.extend(egui::Shape::dashed_line(&[from, to], cut, 2.0, 1.5));
            };
            if sideways {
                for x in [label.left(), label.right()] {
                    painter.line_segment(
                        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                        stroke,
                    );
                }
                dashed(label.left_top(), label.right_top());
                dashed(label.left_bottom(), label.right_bottom());
            } else {
                for y in [label.top(), label.bottom()] {
                    painter.line_segment(
                        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                        stroke,
                    );
                }
                dashed(label.left_top(), label.left_bottom());
                dashed(label.right_top(), label.right_bottom());
            }
        } else {
            painter.rect_stroke(
                label,
                CornerRadius::same(3),
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        painter.text(
            label.center() + egui::vec2(0.0, 0.5),
            egui::Align2::CENTER_CENTER,
            "A",
            semibold(10.0),
            ink,
        );
    })
}

/// One pill with `options` side by side, the `selected` one green; a click anywhere on it
/// moves to the next option. Returns the newly selected index after a click.
pub fn segmented(ui: &mut Ui, options: &[&str], selected: usize) -> Option<usize> {
    let c = palette(ui.ctx());
    let font = medium(14.0);
    let galleys: Vec<_> = options
        .iter()
        .map(|text| {
            ui.painter()
                .layout_no_wrap((*text).to_owned(), font.clone(), c.text)
        })
        .collect();
    let widths: Vec<f32> = galleys
        .iter()
        .map(|galley| galley.size().x + 18.0)
        .collect();
    let height = 26.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(widths.iter().sum::<f32>() + 4.0, height),
        Sense::click(),
    );

    let painter = ui.painter();
    let fill = if response.hovered() {
        c.rule.gamma_multiply(0.85)
    } else {
        c.rule
    };
    painter.rect_filled(rect, CornerRadius::same(13), fill);
    let mut x = rect.left() + 2.0;
    for (index, (galley, width)) in galleys.into_iter().zip(&widths).enumerate() {
        let part = egui::Rect::from_min_size(
            egui::pos2(x, rect.top() + 2.0),
            egui::vec2(*width, height - 4.0),
        );
        let ink = if index == selected {
            painter.rect_filled(part, CornerRadius::same(11), c.green);
            c.green_ink
        } else {
            c.muted
        };
        let at = part.center() - galley.size() / 2.0;
        painter.galley_with_override_text_color(at, galley, ink);
        x += width;
    }

    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
        .then(|| (selected + 1) % options.len().max(1))
}

/// A chip with `leading` drawn before the text (given the ink and fill colours).
fn chip_with(
    ui: &mut Ui,
    text: &str,
    on: bool,
    caret: bool,
    leading: impl FnOnce(&mut Ui, Color32, Color32),
) -> Response {
    let c = palette(ui.ctx());
    let (fill, colour) = if on {
        (c.green, c.green_ink)
    } else {
        (c.rule, c.text)
    };
    let response = egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(13))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                leading(ui, colour, fill);
                ui.label(egui::RichText::new(text).font(medium(14.0)).color(colour));
                if caret {
                    // Drawn: the fonts have no ▾.
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(9.0, 8.0), Sense::hover());
                    let middle = rect.center();
                    ui.painter().add(egui::Shape::convex_polygon(
                        vec![
                            middle + Vec2::new(-4.0, -2.0),
                            middle + Vec2::new(4.0, -2.0),
                            middle + Vec2::new(0.0, 2.5),
                        ],
                        colour,
                        Stroke::NONE,
                    ));
                }
            });
        })
        .response;
    response
        .interact(Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The light/dark button: a round chip with a moon (in light) or a sun (in dark), drawn.
pub fn theme_button(ui: &mut Ui, dark: bool) -> Response {
    let c = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    let fill = if response.hovered() {
        c.green.gamma_multiply(0.35)
    } else {
        c.rule
    };
    let painter = ui.painter();
    let centre = rect.center();
    painter.circle_filled(centre, 13.0, fill);

    if dark {
        // Sun: a disc and eight rays.
        let sun = Color32::from_rgb(0xf5, 0xc4, 0x51);
        painter.circle_filled(centre, 4.0, sun);
        for ray in 0..8 {
            let angle = ray as f32 * std::f32::consts::FRAC_PI_4;
            let direction = Vec2::angled(angle);
            painter.line_segment(
                [centre + direction * 6.5, centre + direction * 9.0],
                Stroke::new(1.6, sun),
            );
        }
    } else {
        // Moon: a disc with a disc of the chip's fill taken out.
        painter.circle_filled(centre, 7.0, c.text);
        painter.circle_filled(centre + egui::vec2(3.8, -3.2), 6.0, fill);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A − or + of a stepper: a round button, dimmed when not `enabled`.
pub fn step(ui: &mut Ui, sign: &str, enabled: bool) -> Response {
    let c = palette(ui.ctx());
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(30.0), sense);
    let fill = if enabled && response.hovered() {
        c.green.gamma_multiply(0.35)
    } else {
        c.rule
    };
    let colour = if enabled { c.text } else { c.faint };
    ui.painter().circle_filled(rect.center(), 14.0, fill);
    ui.painter().text(
        rect.center() - egui::vec2(0.0, 1.0),
        egui::Align2::CENTER_CENTER,
        sign,
        semibold(20.0),
        colour,
    );
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// A key as a keycap: `Ctrl`, `Enter`.
pub fn keycap(ui: &mut Ui, key: &str) {
    let c = palette(ui.ctx());
    egui::Frame::new()
        .fill(c.plate)
        .stroke(Stroke::new(1.0, c.rule))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(5, 0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(key).font(medium(12.5)).color(c.muted));
        });
}

/// Keys and what they do: `[Ctrl] [D] date`.
pub fn shortcut(ui: &mut Ui, keys: &[&str], what: &str) {
    let c = palette(ui.ctx());
    ui.spacing_mut().item_spacing.x = 3.0;
    for key in keys {
        keycap(ui, key);
    }
    ui.add_space(2.0);
    ui.label(egui::RichText::new(what).font(regular(14.0)).color(c.muted));
    ui.add_space(10.0);
}

/// The big green button, with its key under the text; dimmed when not `enabled`.
pub fn primary(ui: &mut Ui, text: &str, key: &str, enabled: bool) -> Response {
    let c = palette(ui.ctx());
    let painter_font = semibold(19.0);
    let key_font = regular(12.5);
    let text_galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), painter_font, c.green_ink);
    let key_galley =
        ui.painter()
            .layout_no_wrap(key.to_owned(), key_font, c.green_ink.gamma_multiply(0.7));
    let width = text_galley.size().x.max(key_galley.size().x) + 48.0;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width.max(150.0), 50.0), sense);

    let fill = match (
        enabled,
        response.is_pointer_button_down_on(),
        response.hovered(),
    ) {
        (false, ..) => c.rule,
        (true, true, _) | (true, _, true) => c.green_pressed,
        _ => c.green,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(10), fill);
    let alpha = if enabled { 1.0 } else { 0.45 };
    let top = rect.center().y - (text_galley.size().y + key_galley.size().y - 4.0) / 2.0;
    let text_pos = egui::pos2(rect.center().x - text_galley.size().x / 2.0, top);
    let key_pos = egui::pos2(
        rect.center().x - key_galley.size().x / 2.0,
        top + text_galley.size().y - 4.0,
    );
    let tint = if enabled { c.green_ink } else { c.muted };
    painter.galley_with_override_text_color(text_pos, text_galley, tint.gamma_multiply(alpha));
    painter.galley_with_override_text_color(key_pos, key_galley, tint.gamma_multiply(0.7 * alpha));

    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// The iotta logo (`assets/label/brands/iotta.png`) `height` high in `colour`.
pub fn logo(ui: &mut Ui, height: f32, colour: Color32) -> Response {
    static LOGO: LazyLock<egui::ColorImage> = LazyLock::new(|| {
        let image =
            image::load_from_memory(include_bytes!("../../../assets/label/brands/iotta.png"))
                .expect("embedded logo is a valid PNG")
                .to_rgba8();
        let (width, height) = (image.width() as usize, image.height() as usize);
        egui::ColorImage {
            size: [width, height],
            source_size: egui::vec2(width as f32, height as f32),
            pixels: image
                .pixels()
                .map(|pixel| Color32::from_white_alpha(pixel[3]))
                .collect(),
        }
    });

    let id = egui::Id::unique("quicklabel_logo");
    let texture = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let texture =
                ui.ctx()
                    .load_texture("iotta-logo", LOGO.clone(), egui::TextureOptions::LINEAR);
            ui.ctx()
                .data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    let aspect = LOGO.size[0] as f32 / LOGO.size[1] as f32;
    ui.add(
        egui::Image::from_texture((texture.id(), egui::vec2(height * aspect, height))).tint(colour),
    )
}

/// The window icon: the label on iotta green (`assets/icon.png`, also the exe's icon).
#[must_use]
pub fn window_icon() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!("../assets/icon.png"))
        .expect("embedded icon is a valid PNG")
        .to_rgba8();
    egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}
