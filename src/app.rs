//! The window: text field, preview, printer, keys.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chopshop_i18n::{Language, t, tf};
use chopshop_label::TextLabel;
use chopshop_printer::Media;
use egui::text::{CCursor, CCursorRange};
use egui::{Color32, CornerRadius, Key, Modifiers, RichText, Ui};

use crate::printer::{Link, State};
use crate::settings::{self, Settings};
use crate::style;

/// Tape assumed for the preview while no printer reports one.
const DEFAULT_MEDIA: &str = "62";

/// The text field's id, for focus and its cursor.
const FIELD: &str = "quicklabel_text";

struct App {
    settings: Settings,
    printers: Vec<Link>,
    /// The printer picked by a click; else the first that can print.
    chosen: Option<usize>,
    text: String,
    preview: Option<(String, egui::TextureHandle)>,
    message: Option<Result<String, String>>,
    printing: bool,
    close_at: Option<Instant>,
    focused: bool,
    show_printers: bool,
    new_printer: String,
    screenshot: Option<(PathBuf, Instant)>,
}

/// Runs the window until it closes.
/// `theme` overrides the system's light or dark.
pub fn run(
    text: Option<String>,
    screenshot: Option<PathBuf>,
    theme: Option<egui::Theme>,
) -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Quick label")
        .with_icon(std::sync::Arc::new(style::window_icon()))
        .with_inner_size([680.0, 540.0])
        .with_min_inner_size([480.0, 400.0])
        .with_always_on_top();
    let native = eframe::NativeOptions {
        viewport,
        centered: true,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "quicklabel",
        native,
        Box::new(move |creation| {
            style::install(&creation.egui_ctx);
            let settings = Settings::load();
            let saved = match settings.theme.as_str() {
                "light" => Some(egui::Theme::Light),
                "dark" => Some(egui::Theme::Dark),
                _ => None,
            };
            if let Some(theme) = theme.or(saved) {
                creation.egui_ctx.set_theme(theme);
            }
            chopshop_i18n::set_language(if settings.language.is_empty() {
                settings::system_language()
            } else {
                Language::from_code(&settings.language)
            });

            let ctx = creation.egui_ctx.clone();
            let printers = settings
                .printers
                .iter()
                .map(|host| start_link(host, &ctx))
                .collect();
            let show_printers = settings.printers.is_empty();

            Ok(Box::new(App {
                settings,
                printers,
                chosen: None,
                text: text.unwrap_or_default(),
                preview: None,
                message: None,
                printing: false,
                close_at: None,
                focused: false,
                show_printers,
                new_printer: String::new(),
                screenshot: screenshot.map(|path| (path, Instant::now())),
            }))
        }),
    )
}

fn start_link(host: &str, ctx: &egui::Context) -> Link {
    let ctx = ctx.clone();
    Link::start(host.to_owned(), move || ctx.request_repaint())
}

impl App {
    /// The printer to use and its tape, when it can print.
    fn target(&self) -> Option<(usize, &'static Media)> {
        let ready = |index: usize| {
            self.printers[index]
                .state()
                .ready_media()
                .map(|media| (index, media))
        };
        match self.chosen {
            Some(index) if index < self.printers.len() => ready(index),
            _ => (0..self.printers.len()).find_map(ready),
        }
    }

    /// The tape the label is laid out for: the target's, else 62 mm.
    fn media(&self) -> &'static Media {
        self.target()
            .map(|(_, media)| media)
            .unwrap_or_else(|| Media::by_id(DEFAULT_MEDIA).expect("62 mm is in the media table"))
    }

    fn render(&self, media: &Media) -> chopshop_label::Bitmap {
        let length = (!media.is_continuous()).then_some(u32::from(media.print_length_dots));
        TextLabel::new(&self.text).render(u32::from(media.print_pins), length)
    }

    fn print(&mut self) {
        if TextLabel::new(&self.text).is_empty() || self.printing {
            return;
        }

        if self.printers.is_empty() {
            let bitmap = self.render(self.media());
            let dir = Settings::labels_dir();
            let path = dir.join(format!(
                "label_{}.png",
                jiff::Zoned::now().strftime("%Y-%m-%d_%H.%M.%S")
            ));
            let saved = std::fs::create_dir_all(&dir)
                .map_err(|error| error.to_string())
                .and_then(|()| {
                    bitmap
                        .to_gray_image()
                        .save(&path)
                        .map_err(|error| error.to_string())
                });
            self.message = Some(
                saved.map(|()| tf("No printer: saved as {path}", &[("path", &path.display())])),
            );
            return;
        }

        let Some((index, media)) = self.target() else {
            self.message = Some(Err(t("No printer ready with tape loaded").to_owned()));
            return;
        };
        let bitmap = self.render(media);
        self.printers[index].print(bitmap, media);
        self.printing = true;
        self.message = None;
    }

    /// Inserts `text` at the cursor (replacing a selection).
    fn insert(&mut self, ctx: &egui::Context, text: &str) {
        let id = egui::Id::unique(FIELD);
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        let end = self.text.chars().count();
        let range = state
            .cursor
            .char_range()
            .unwrap_or_else(|| CCursorRange::one(CCursor::new(end)));
        let (start, stop) = (
            range.primary.index.0.min(range.secondary.index.0),
            range.primary.index.0.max(range.secondary.index.0),
        );
        let byte = |index: usize| {
            self.text
                .char_indices()
                .nth(index)
                .map_or(self.text.len(), |(byte, _)| byte)
        };
        let (from, to) = (byte(start), byte(stop));
        self.text.replace_range(from..to, text);
        let caret = CCursor::new(start + text.chars().count());
        state.cursor.set_char_range(Some(CCursorRange::one(caret)));
        state.store(ctx, id);
    }

    fn keys(&mut self, ctx: &egui::Context) {
        // Before the text field, which would take Enter.
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::Enter)) {
            self.print();
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::D)) {
            let today = jiff::Zoned::now().strftime("%d-%m-%Y").to_string();
            self.insert(ctx, &today);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::T)) {
            let now = jiff::Zoned::now().strftime("%H:%M").to_string();
            self.insert(ctx, &now);
        }
    }

    fn results(&mut self, ctx: &egui::Context) {
        for link in &self.printers {
            for result in link.results() {
                self.printing = false;
                match result {
                    Ok(printer) => {
                        self.message =
                            Some(Ok(tf("Printed on {printer}", &[("printer", &printer)])));
                        self.close_at = Some(Instant::now() + Duration::from_millis(600));
                    }
                    Err(reason) => self.message = Some(Err(reason)),
                }
            }
        }
        if let Some(at) = self.close_at {
            if Instant::now() >= at {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }
    }

    /// The label as it comes out: white paper on a tray, with its size under it; the last
    /// print's outcome as a pill on the tray.
    fn preview(&mut self, ui: &mut Ui) {
        let c = style::palette(ui.ctx());
        let media = self.media();
        let label = TextLabel::new(&self.text);
        let key = format!("{}|{}", media.id, self.text);
        if self.preview.as_ref().is_none_or(|(old, _)| *old != key) {
            let length = (!media.is_continuous()).then_some(u32::from(media.print_length_dots));
            let image = label
                .landscape(u32::from(media.print_pins), length)
                .to_gray_image();
            let color = egui::ColorImage::from_gray(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            );
            let texture =
                ui.ctx()
                    .load_texture("quicklabel_preview", color, egui::TextureOptions::LINEAR);
            self.preview = Some((key, texture));
        }
        let Some((_, texture)) = &self.preview else {
            return;
        };

        let tray = ui.available_rect_before_wrap();
        ui.allocate_rect(tray, egui::Sense::hover());
        let painter = ui.painter_at(tray);
        painter.rect_filled(tray, CornerRadius::same(12), c.tray);

        // The paper: the print area plus the tape's unprinted edges (and the feed of
        // continuous tape), in dots.
        let [w, h] = texture.size();
        let (w, h) = (w as f32, h as f32);
        let continuous = media.is_continuous();
        let edge = |pins: u16| f32::from(pins).max(12.0);
        let feed = if continuous { 35.0 } else { 18.0 };
        let across = h + edge(media.left_pins.min(media.right_pins)) * 2.0;
        // Nothing typed: a blank label of a square-ish length to type onto.
        let along = if label.is_empty() {
            across * 1.4
        } else {
            w + 2.0 * feed
        };
        let paper_dots = egui::vec2(along, across);
        let caption_height = 22.0;
        let room = tray.shrink(22.0);
        let scale = (room.width() / paper_dots.x)
            .min((room.height() - caption_height) / paper_dots.y)
            .min(0.6);
        let paper_size = paper_dots * scale;
        let paper = egui::Rect::from_center_size(
            room.center() - egui::vec2(0.0, caption_height / 2.0),
            paper_size,
        );
        let corner = if continuous {
            1
        } else {
            (paper_size.y * 0.04).clamp(3.0, 10.0) as u8
        };
        let shadow = egui::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(if c.dark { 120 } else { 38 }),
        };
        painter.add(shadow.as_shape(paper, CornerRadius::same(corner)));
        painter.rect_filled(paper, CornerRadius::same(corner), Color32::WHITE);

        if label.is_empty() {
            painter.text(
                paper.center(),
                egui::Align2::CENTER_CENTER,
                t("Your text appears here"),
                style::regular((paper_size.y * 0.12).clamp(13.0, 22.0)),
                style::LIGHT.faint,
            );
        } else {
            let print = egui::Rect::from_center_size(paper.center(), egui::vec2(w, h) * scale);
            egui::Image::from_texture((texture.id(), print.size())).paint_at(ui, print);
        }

        let length_mm = if continuous {
            ((w + 2.0 * feed) / 300.0 * 25.4).round()
        } else {
            f32::from(media.length_mm)
        };
        let caption = if label.is_empty() {
            tf("{width} mm tape", &[("width", &media.width_mm)])
        } else if continuous {
            tf(
                "{width} mm tape · {length} mm long",
                &[("width", &media.width_mm), ("length", &length_mm)],
            )
        } else {
            tf(
                "{width} × {length} mm label",
                &[("width", &media.width_mm), ("length", &length_mm)],
            )
        };
        painter.text(
            egui::pos2(paper.center().x, paper.bottom() + 10.0),
            egui::Align2::CENTER_TOP,
            caption,
            style::regular(13.5),
            c.muted,
        );

        // The outcome, as a pill at the tray's top.
        let pill = match &self.message {
            Some(Ok(text)) => Some((format!("✔  {text}"), c.green, c.green_ink)),
            Some(Err(text)) => Some((text.clone(), c.alarm, Color32::WHITE)),
            None if self.printing => Some((t("Printing…").to_owned(), c.plate, c.text)),
            None => None,
        };
        if let Some((text, fill, colour)) = pill {
            let galley = painter.layout(text, style::medium(15.0), colour, tray.width() - 60.0);
            let size = galley.size() + egui::vec2(28.0, 12.0);
            let rect = egui::Rect::from_center_size(
                egui::pos2(tray.center().x, tray.top() + 14.0 + size.y / 2.0),
                size,
            );
            painter.add(shadow.as_shape(rect, CornerRadius::same(16)));
            painter.rect_filled(rect, CornerRadius::same(16), fill);
            painter.galley(rect.min + egui::vec2(14.0, 6.0), galley, colour);
        }
    }

    /// The printers as chips with a status dot: the one used is green; a click picks another.
    fn printer_line(&mut self, ui: &mut Ui) {
        let c = style::palette(ui.ctx());
        if self.printers.is_empty() {
            if style::chip(ui, t("No printer yet: add one"), false, Some(c.echo))
                .on_hover_text(t("Without a printer, the label is saved as a PNG"))
                .clicked()
            {
                self.show_printers = true;
            }
            return;
        }

        let target = self.target().map(|(index, _)| index);
        ui.horizontal_wrapped(|ui| {
            for index in 0..self.printers.len() {
                let link = &self.printers[index];
                let state = link.state();
                let (text, dot) = describe(&link.host, &state, c);
                if style::chip(ui, &text, target == Some(index), Some(dot))
                    .on_hover_text(t("Print on this printer"))
                    .clicked()
                {
                    self.chosen = Some(index);
                }
            }
        });
    }

    /// The language switch: EN / NL.
    fn language_switch(&mut self, ui: &mut Ui) {
        for language in Language::ALL.into_iter().rev() {
            let on = chopshop_i18n::language() == language;
            if style::chip(ui, language.short(), on, None).clicked() && !on {
                chopshop_i18n::set_language(language);
                self.settings.language = language.code().to_owned();
                self.save();
            }
        }
    }

    /// Light or dark: a moon in light (to dark), a sun in dark (to light); remembered.
    fn theme_switch(&mut self, ui: &mut Ui) {
        let dark = ui.ctx().theme() == egui::Theme::Dark;
        let hint = if dark {
            t("Switch to light")
        } else {
            t("Switch to dark")
        };
        if style::theme_button(ui, dark).on_hover_text(hint).clicked() {
            let (theme, code) = if dark {
                (egui::Theme::Light, "light")
            } else {
                (egui::Theme::Dark, "dark")
            };
            ui.ctx().set_theme(theme);
            self.settings.theme = code.to_owned();
            self.save();
        }
    }

    fn save(&mut self) {
        if let Err(error) = self.settings.save() {
            self.message = Some(Err(tf(
                "Saving the settings failed: {error}",
                &[("error", &error)],
            )));
        }
    }

    /// Printers to add and remove, on a card in the tray's place; open on its own while
    /// there are none.
    fn printers_card(&mut self, ui: &mut Ui) {
        let c = style::palette(ui.ctx());
        let ctx = ui.ctx().clone();
        let mut changed = false;
        let rect = ui.available_rect_before_wrap();

        let mut card = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        egui::Frame::new()
            .fill(c.plate)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::same(18))
            .show(&mut card, |ui| {
                ui.set_min_size(rect.size() - egui::Vec2::splat(36.0));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("Printers")).font(style::semibold(19.0)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t("Done")).clicked() {
                            self.show_printers = false;
                        }
                    });
                });
                ui.label(
                    RichText::new(t(
                        "Brother QL printers on the network, by address. A label goes to the first that is ready with tape in; click one below the label to pick it.",
                    ))
                    .font(style::regular(14.0))
                    .color(c.muted),
                );
                ui.add_space(10.0);

                let mut remove = None;
                for (index, link) in self.printers.iter().enumerate() {
                    let (text, dot) = describe(&link.host, &link.state(), c);
                    ui.horizontal(|ui| {
                        let (spot, _) =
                            ui.allocate_exact_size(egui::Vec2::splat(10.0), egui::Sense::hover());
                        ui.painter().circle_filled(spot.center(), 4.5, dot);
                        ui.label(RichText::new(&link.host).font(style::medium(16.0)));
                        if text != link.host {
                            ui.label(
                                RichText::new(&text)
                                    .font(style::regular(14.0))
                                    .color(c.muted),
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(t("Remove"))
                                .on_hover_text(t("Remove this printer"))
                                .clicked()
                            {
                                remove = Some(index);
                            }
                        });
                    });
                }
                if let Some(index) = remove {
                    self.settings.printers.remove(index);
                    self.printers.remove(index);
                    self.chosen = None;
                    changed = true;
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let field = egui::Frame::new()
                        .fill(c.ink)
                        .stroke(egui::Stroke::new(1.0, c.rule))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(8, 5))
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_printer)
                                    .frame(egui::Frame::NONE)
                                    .hint_text(t("Printer address, e.g. 192.168.1.40"))
                                    .font(style::regular(16.0))
                                    .desired_width(240.0),
                            )
                        })
                        .inner;
                    let entered =
                        field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
                    let host = self.new_printer.trim().to_owned();
                    if (ui.button(t("Add")).clicked() || entered)
                        && !host.is_empty()
                        && !self.settings.printers.contains(&host)
                    {
                        self.printers.push(start_link(&host, &ctx));
                        self.settings.printers.push(host);
                        self.new_printer.clear();
                        changed = true;
                    }
                });
            });

        if changed {
            self.save();
        }
    }

    fn take_screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, started)) = &self.screenshot else {
            return;
        };
        if started.elapsed() < Duration::from_millis(1500) {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        let path = path.clone();
        self.screenshot = None;
        let closer = ctx.clone();
        ctx.request_screenshot(move |image| {
            let [width, height] = image.size;
            let rgba: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
            if let Some(buffer) = image::RgbaImage::from_raw(width as u32, height as u32, rgba)
                && buffer.save(&path).is_ok()
            {
                println!("screenshot: {}", path.display());
            }
            closer.send_viewport_cmd(egui::ViewportCommand::Close);
        });
    }
}

/// A printer's chip text and status dot.
fn describe(host: &str, state: &State, c: &style::Palette) -> (String, Color32) {
    match state {
        State::Online { status, name } => (
            format!(
                "{} · {}",
                name.clone().unwrap_or_else(|| host.to_owned()),
                status.media_description()
            ),
            if state.ready_media().is_some() {
                c.accent
            } else {
                c.alarm
            },
        ),
        State::Offline => (tf("{host}: doesn't answer", &[("host", &host)]), c.alarm),
        State::Checking => (tf("{host}: checking…", &[("host", &host)]), c.echo),
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let c = style::palette(&ctx);
        self.results(&ctx);
        self.keys(&ctx);

        egui::Panel::top("quicklabel_header")
            .frame(
                egui::Frame::new()
                    .fill(c.plate)
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show_separator_line(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    style::logo(ui, 20.0, c.green);
                    ui.add_space(6.0);
                    let (bar, _) =
                        ui.allocate_exact_size(egui::vec2(1.0, 22.0), egui::Sense::hover());
                    ui.painter().rect_filled(bar, CornerRadius::ZERO, c.rule);
                    ui.add_space(6.0);
                    ui.label(RichText::new(t("Quick label")).font(style::semibold(21.0)));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        self.language_switch(ui);
                        ui.add_space(6.0);
                        self.theme_switch(ui);
                        ui.add_space(6.0);
                        if style::chip(ui, t("Printers"), self.show_printers, None)
                            .on_hover_text(t("Add or remove printers"))
                            .clicked()
                        {
                            self.show_printers = !self.show_printers;
                        }
                    });
                });
            });

        egui::Panel::bottom("quicklabel_footer")
            .frame(egui::Frame::new().fill(c.ink).inner_margin(egui::Margin {
                left: 18,
                right: 18,
                top: 4,
                bottom: 16,
            }))
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let label = if self.printers.is_empty() {
                        t("Save as PNG")
                    } else {
                        t("Print")
                    };
                    let ready = !TextLabel::new(&self.text).is_empty()
                        && !self.printing
                        && (self.printers.is_empty() || self.target().is_some());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if style::primary(ui, label, "Ctrl + Enter", ready).clicked() {
                            self.print();
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            self.printer_line(ui);
                        });
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(c.ink).inner_margin(egui::Margin {
                left: 18,
                right: 18,
                top: 16,
                bottom: 12,
            }))
            .show(ui, |ui| {
                let id = egui::Id::unique(FIELD);
                let focused = ctx.memory(|memory| memory.has_focus(id));
                egui::Frame::new()
                    .fill(c.plate)
                    .stroke(egui::Stroke::new(
                        if focused { 1.5 } else { 1.0 },
                        if focused { c.green } else { c.rule },
                    ))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        let field = egui::TextEdit::multiline(&mut self.text)
                            .id(id)
                            .frame(egui::Frame::NONE)
                            .hint_text(t("Type the label text"))
                            .font(style::regular(24.0))
                            .desired_rows(3)
                            .desired_width(f32::INFINITY);
                        let response = ui.add(field);
                        if !self.focused {
                            response.request_focus();
                            self.focused = true;
                        }
                    });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.spacing_mut().item_spacing.x = 1.0;
                    let hint = |text: &str| {
                        RichText::new(text)
                            .font(style::regular(14.0))
                            .color(c.muted)
                    };
                    ui.label(hint("**").color(c.faint));
                    ui.label(
                        RichText::new(t("bold"))
                            .font(style::semibold(14.0))
                            .color(c.muted),
                    );
                    ui.label(hint("**").color(c.faint));
                    ui.add_space(10.0);
                    ui.label(hint("*").color(c.faint));
                    ui.label(hint(t("italic")).italics());
                    ui.label(hint("*").color(c.faint));
                    ui.add_space(12.0);
                    style::shortcut(ui, &["Ctrl", "D"], t("date"));
                    style::shortcut(ui, &["Ctrl", "T"], t("time"));
                    style::shortcut(ui, &["Esc"], t("close"));
                });
                ui.add_space(10.0);

                if self.show_printers {
                    self.printers_card(ui);
                } else {
                    self.preview(ui);
                }
            });

        self.take_screenshot(&ctx);
    }
}
