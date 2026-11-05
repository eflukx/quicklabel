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
pub fn run(text: Option<String>, screenshot: Option<PathBuf>) -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Quick label")
        .with_inner_size([640.0, 460.0])
        .with_min_inner_size([420.0, 320.0])
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

    fn preview(&mut self, ui: &mut Ui) {
        let media = self.media();
        let key = format!("{}|{}", media.id, self.text);
        if self.preview.as_ref().is_none_or(|(old, _)| *old != key) {
            let length = (!media.is_continuous()).then_some(u32::from(media.print_length_dots));
            let image = TextLabel::new(&self.text)
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

        let [w, h] = texture.size();
        let (w, h) = (w as f32, h as f32);
        let margin = 6.0;
        let scale = ((ui.available_width() - 2.0 * margin) / w).min(140.0 / h);
        let size = egui::vec2(w * scale, h * scale);
        let (rect, _) =
            ui.allocate_exact_size(size + egui::Vec2::splat(2.0 * margin), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), Color32::WHITE);
        egui::Image::from_texture((texture.id(), size)).paint_at(
            ui,
            egui::Rect::from_min_size(rect.min + egui::Vec2::splat(margin), size),
        );
    }

    /// The printers as badges: the one used is green; a click picks another.
    fn printer_line(&mut self, ui: &mut Ui) {
        let small = |text: String| RichText::new(text).font(style::regular(14.0));
        if self.printers.is_empty() {
            ui.label(
                small(t("No printer yet: add its address under Printers").to_owned())
                    .color(style::MUTED),
            );
            return;
        }

        let target = self.target().map(|(index, _)| index);
        ui.horizontal_wrapped(|ui| {
            for index in 0..self.printers.len() {
                let link = &self.printers[index];
                let text = match link.state() {
                    State::Online { status, name } => format!(
                        "{} · {}",
                        name.unwrap_or_else(|| link.host.clone()),
                        status.media_description()
                    ),
                    State::Offline => tf("{host}: doesn't answer", &[("host", &link.host)]),
                    State::Checking => tf("{host}: checking…", &[("host", &link.host)]),
                };
                if style::badge(ui, &text, target == Some(index))
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
            if style::badge(ui, language.short(), on).clicked() && !on {
                chopshop_i18n::set_language(language);
                self.settings.language = language.code().to_owned();
                self.save();
            }
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

    /// Printers to add and remove; open on its own while there are none.
    fn printers_section(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let mut changed = false;

        egui::CollapsingHeader::new(RichText::new(t("Printers")).color(style::MUTED))
            .id_salt("quicklabel_printers")
            .default_open(self.show_printers)
            .show(ui, |ui| {
                let mut remove = None;
                for (index, host) in self.settings.printers.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(host).font(style::regular(15.0)));
                        if ui
                            .small_button("×")
                            .on_hover_text(t("Remove this printer"))
                            .clicked()
                        {
                            remove = Some(index);
                        }
                    });
                }
                if let Some(index) = remove {
                    self.settings.printers.remove(index);
                    self.printers.remove(index);
                    self.chosen = None;
                    changed = true;
                }

                ui.horizontal(|ui| {
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.new_printer)
                            .hint_text(t("Printer address, e.g. 192.168.1.40"))
                            .desired_width(240.0),
                    );
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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.results(&ctx);
        self.keys(&ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(style::INK).inner_margin(egui::Margin::same(16)))
            .show(ui, |ui| {
                let field = egui::TextEdit::multiline(&mut self.text)
                    .id(egui::Id::unique(FIELD))
                    .hint_text(t("Type the label text"))
                    .font(style::regular(22.0))
                    .desired_rows(3)
                    .desired_width(f32::INFINITY);
                let response = ui.add(field);
                if !self.focused {
                    response.request_focus();
                    self.focused = true;
                }

                ui.add_space(10.0);
                self.preview(ui);
                ui.add_space(8.0);
                self.printer_line(ui);

                match &self.message {
                    Some(Ok(text)) => {
                        ui.label(RichText::new(text).color(style::ACCENT));
                    }
                    Some(Err(text)) => {
                        ui.label(RichText::new(text).color(style::ALARM));
                    }
                    None if self.printing => {
                        ui.label(RichText::new(t("Printing…")).color(style::MUTED));
                    }
                    None => {}
                }

                ui.add_space(6.0);
                self.printers_section(ui);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(t(
                                "Ctrl+Enter prints  ·  **bold**  *italic*  ·  Ctrl+D date  ·  Ctrl+T time  ·  Esc closes",
                            ))
                            .font(style::regular(14.0))
                            .color(style::FAINT),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            self.language_switch(ui);
                        });
                    });
                });
            });

        self.take_screenshot(&ctx);
    }
}
