//! The window: text field, preview, printer, keys.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::i18n::{Language, t, tf};
use egui::text::{CCursor, CCursorRange};
use egui::{Color32, CornerRadius, Key, Modifiers, RichText, Ui};
use ql_raster::Media;
use ql_raster::job::Resolution;
use ql_raster::label::{Length, Run, TextLabel, Typeface};

use crate::printer::{Link, State};
use crate::settings::{self, Cut, Layout, Settings};
use crate::{markdown, numbering, resident, style};

/// Tape assumed for the preview while no printer reports one.
const DEFAULT_MEDIA: &str = "62";

/// Dots per millimetre at 300 dpi.
const DOTS_PER_MM: f32 = 300.0 / 25.4;

/// How long a success message shows before it fades (in [`MESSAGE_FADE`]).
const MESSAGE_SHOWN: Duration = Duration::from_secs(5);
const MESSAGE_FADE: Duration = Duration::from_millis(600);

/// Feed before and after a label on continuous tape, in dots.
const FEED_DOTS: u32 = 35;

/// The length limits offered for continuous tape: times the tape's width; 0: none.
const LENGTHS: [u8; 4] = [0, 1, 2, 3];

/// The most copies in one go.
const MAX_COPIES: u16 = 99;

/// The number of labels' field (Ctrl+N), for focus.
const COPIES_FIELD: &str = "copies";

/// The text field's id, for focus and its cursor.
const FIELD: &str = "quicklabel_text";

/// The preview: the label at full resolution as a texture, shown filtered at any smaller size.
struct Preview {
    /// Tape, layout and text it was made for.
    key: String,
    /// The label, a pixel per dot.
    image: image::GrayImage,
    /// Shrunk to the screen pixels shown (Catmull-Rom: sharp), with that size.
    texture: Option<([u32; 2], egui::TextureHandle)>,
}

struct App {
    settings: Settings,
    printers: Vec<Link>,
    /// The printer picked by a click; else the first that can print.
    chosen: Option<usize>,
    text: String,
    preview: Option<Preview>,
    message: Option<Result<String, String>>,
    /// The message last seen and since when: a success fades after [`MESSAGE_SHOWN`].
    message_since: Option<(Result<String, String>, Instant)>,
    printing: bool,
    /// How many labels Print makes; back to 1 each start.
    copies: u16,
    /// The number of labels as being typed (Ctrl+N or a click on it).
    copies_typed: Option<String>,
    /// Whether the resident process runs, and when that was asked.
    resident_running: Option<(bool, Instant)>,
    /// Why switching resident mode failed.
    resident_error: Option<String>,
    /// The printer search running, and its outcome: the networks searched and what answered.
    search: Option<std::sync::mpsc::Receiver<Vec<Found>>>,
    /// USB printers plugged in, and when that was looked at.
    usb_found: Option<(Instant, Vec<String>)>,
    searched: Option<(Vec<String>, Vec<Found>)>,
    close_at: Option<Instant>,
    focused: bool,
    show_printers: bool,
    new_printer: String,
    screenshot: Option<(PathBuf, Instant)>,
}

/// A printer the search found: its address (`usb:<serial>` on USB) and model.
type Found = (String, String);

/// Runs the window until it closes.
/// `theme` overrides the system's light or dark.
pub fn run(
    text: Option<String>,
    screenshot: Option<PathBuf>,
    theme: Option<egui::Theme>,
    window: Option<std::net::TcpListener>,
) -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Quick label")
        .with_icon(std::sync::Arc::new(style::window_icon()))
        .with_inner_size([680.0, 600.0])
        .with_min_inner_size([620.0, 440.0]);
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
            crate::i18n::set_language(if settings.language.is_empty() {
                settings::system_language()
            } else {
                Language::from_code(&settings.language)
            });

            let ctx = creation.egui_ctx.clone();
            if let Some(listener) = window {
                let ctx = ctx.clone();
                resident::serve(listener, move |message| {
                    if message == "show" {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        ctx.request_repaint();
                    }
                });
            }
            let printers = settings
                .printers
                .iter()
                .map(|host| start_link(host, &ctx))
                .collect();
            let show_printers = settings.printers.is_empty();
            let text = text.unwrap_or_else(|| settings.last_text.clone());

            Ok(Box::new(App {
                settings,
                printers,
                chosen: None,
                text,
                preview: None,
                message: None,
                message_since: None,
                printing: false,
                copies: 1,
                copies_typed: None,
                resident_running: None,
                resident_error: None,
                search: None,
                usb_found: None,
                searched: None,
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
    /// The `QUICKLABEL_TAPE` variable (a media id such as `62x29`) overrides the 62 mm, for
    /// screenshots of other tapes.
    fn media(&self) -> &'static Media {
        self.target().map(|(_, media)| media).unwrap_or_else(|| {
            std::env::var("QUICKLABEL_TAPE")
                .ok()
                .and_then(|id| Media::by_id(&id))
                .unwrap_or_else(|| {
                    Media::by_id(DEFAULT_MEDIA).expect("62 mm is in the media table")
                })
        })
    }

    /// The layout kept for `media`'s tape.
    fn layout(&self, media: &Media) -> Layout {
        self.settings
            .layouts
            .get(media.id)
            .copied()
            .unwrap_or_default()
    }

    fn set_layout(&mut self, media: &Media, layout: Layout) {
        self.settings.layouts.insert(media.id.to_owned(), layout);
        self.save();
    }

    /// The raster of label `index` of the run (`{n}` filled in).
    fn render(
        &self,
        media: &Media,
        index: u16,
        resolution: Resolution,
    ) -> ql_raster::label::Bitmap {
        let layout = self.layout(media);
        let (length, run) = shape(media, layout);
        let label = TextLabel::new(&numbering::fill(&self.text, index))
            .fill_lines(layout.fill_lines)
            .typeface(self.settings.typeface());
        let across = u32::from(media.print_pins);
        match resolution {
            Resolution::Normal => label.render(across, length, run),
            Resolution::High => label.render_fine(across, length, run),
        }
    }

    /// Every label of the run.
    fn render_all(&self, media: &Media, resolution: Resolution) -> Vec<ql_raster::label::Bitmap> {
        (0..self.copies)
            .map(|index| self.render(media, index, resolution))
            .collect()
    }

    /// The printer `index`'s resolution: high when set (the default) and its model has it.
    fn resolution(&self, index: usize) -> Resolution {
        let link = &self.printers[index];
        if self.settings.printer(&link.host).high_resolution && link.has_high_resolution() {
            Resolution::High
        } else {
            Resolution::Normal
        }
    }

    fn print(&mut self) {
        if TextLabel::new(&self.text).is_empty() || self.printing {
            return;
        }
        self.keep_text();

        if self.printers.is_empty() {
            let labels = self.render_all(self.media(), Resolution::Normal);
            let dir = Settings::labels_dir();
            let stamp = jiff::Zoned::now().strftime("%Y-%m-%d_%H.%M.%S").to_string();
            let mut saved = std::fs::create_dir_all(&dir)
                .map_err(|error| error.to_string())
                .map(|()| dir.join(format!("label_{stamp}.png")));
            for (index, bitmap) in labels.iter().enumerate() {
                let name = if labels.len() > 1 {
                    format!("label_{stamp}_{}.png", index + 1)
                } else {
                    format!("label_{stamp}.png")
                };
                let path = dir.join(name);
                saved = saved.and_then(|_| {
                    bitmap
                        .to_gray_image()
                        .save(&path)
                        .map(|()| path.clone())
                        .map_err(|error| error.to_string())
                });
            }
            self.message = Some(
                saved.map(|path| tf("No printer: saved as {path}", &[("path", &path.display())])),
            );
            return;
        }

        let Some((index, media)) = self.target() else {
            self.message = Some(Err(t("No printer ready with tape loaded").to_owned()));
            return;
        };
        let resolution = self.resolution(index);
        let labels = self.render_all(media, resolution);
        let cut = self.layout(media).cut.job();
        self.printers[index].print(labels, media, cut, resolution);
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

    /// Bold or italic on or off around the field's selection (Ctrl+B, Ctrl+I).
    fn emphasis(&mut self, ctx: &egui::Context, style: markdown::Style) {
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
        let (text, selected) = markdown::toggle(&self.text, start..stop, style);
        self.text = text;
        state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(selected.start),
            CCursor::new(selected.end),
        )));
        state.store(ctx, id);
        ctx.memory_mut(|memory| memory.request_focus(id));
    }

    /// The number of labels into its field, selected: typing replaces it.
    fn type_copies(&mut self, ctx: &egui::Context) {
        let typed = self.copies.to_string();
        let id = egui::Id::unique(COPIES_FIELD);
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(0),
            CCursor::new(typed.len()),
        )));
        state.store(ctx, id);
        self.copies_typed = Some(typed);
        ctx.memory_mut(|memory| memory.request_focus(id));
    }

    fn keys(&mut self, ctx: &egui::Context) {
        // "/" with nothing focused: back to the text (on any keyboard layout: the typed
        // character counts, not the key).
        let nothing_focused = ctx.memory(|memory| memory.focused().is_none());
        if nothing_focused {
            let slash = ctx.input_mut(|input| {
                let typed =
                    |event: &egui::Event| matches!(event, egui::Event::Text(text) if text == "/");
                let found = input.events.iter().any(typed);
                input.events.retain(|event| !typed(event));
                found
            });
            if slash {
                ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique(FIELD)));
            }
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::B)) {
            self.emphasis(ctx, markdown::Style::Bold);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::I)) {
            self.emphasis(ctx, markdown::Style::Italic);
        }

        // Before the text field, which would take Enter.
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::Enter)) {
            self.print();
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::N)) {
            self.type_copies(ctx);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
            // Typing the number: back to the text, else close.
            if self.copies_typed.is_some() {
                self.copies_typed = None;
                ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique(FIELD)));
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::D)) {
            let today = jiff::Zoned::now().strftime("%d-%m-%Y").to_string();
            self.insert(ctx, &today);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::ArrowUp)) {
            self.copies = (self.copies + 1).min(MAX_COPIES);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::ArrowDown)) {
            self.copies = self.copies.saturating_sub(1).max(1);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::R)) {
            let media = self.media();
            let layout = self.layout(media);
            self.set_layout(
                media,
                Layout {
                    sideways: !layout.sideways,
                    ..layout
                },
            );
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::F)) {
            let media = self.media();
            let layout = self.layout(media);
            self.set_layout(
                media,
                Layout {
                    fill_lines: !layout.fill_lines,
                    ..layout
                },
            );
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
                        self.message = Some(Ok(if self.copies > 1 {
                            tf(
                                "Printed {copies} × on {printer}",
                                &[("copies", &self.copies), ("printer", &printer)],
                            )
                        } else {
                            tf("Printed on {printer}", &[("printer", &printer)])
                        }));
                        if self.settings.close_after_print {
                            self.close_at = Some(Instant::now() + Duration::from_millis(600));
                        }
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
        let layout = self.layout(media);
        let typeface = self.settings.typeface();
        let label = TextLabel::new(&numbering::fill(&self.text, 0))
            .fill_lines(layout.fill_lines)
            .typeface(typeface);
        let (length, run) = shape(media, layout);
        let key = format!("{}|{layout:?}|{typeface:?}|{}", media.id, self.text);
        if self
            .preview
            .as_ref()
            .is_none_or(|preview| preview.key != key)
        {
            let image = label
                .landscape(u32::from(media.print_pins), length, run)
                .to_gray_image();
            self.preview = Some(Preview {
                key,
                image,
                texture: None,
            });
        }
        let Some((w, h)) = self
            .preview
            .as_ref()
            .map(|preview| (preview.image.width() as f32, preview.image.height() as f32))
        else {
            return;
        };

        let tray = ui.available_rect_before_wrap();
        ui.allocate_rect(tray, egui::Sense::hover());
        let painter = ui.painter_at(tray);
        painter.rect_filled(tray, CornerRadius::same(12), c.tray);
        self.layout_bar(ui, tray, media, layout);

        // The paper: the print area plus the tape's unprinted edges and the feed (or the
        // die-cut label's edge), in dots; the tape runs left to right along it, else down.
        let continuous = media.is_continuous();
        let edge = f32::from(media.left_pins.min(media.right_pins)).max(12.0);
        let feed = if continuous { FEED_DOTS as f32 } else { 18.0 };
        let along_tape = run == Run::AlongTape;
        let (print_along, print_across) = if along_tape { (w, h) } else { (h, w) };
        let across = print_across + 2.0 * edge;
        let along = if label.is_empty() {
            // Nothing typed: the longest the label gets, or a square-ish one.
            match length {
                Length::Fixed(lines) | Length::AtMost(lines) => lines as f32 + 2.0 * feed,
                Length::Free => across * 1.4,
            }
        } else {
            print_along + 2.0 * feed
        };
        let paper_dots = if along_tape {
            egui::vec2(along, across)
        } else {
            egui::vec2(across, along)
        };
        // Room for the dimension lines: under the paper and right of it (and as much on the
        // left, so the paper stays centred).
        let (below, beside) = (26.0, 32.0);
        // As large as the tray allows, but never a dot larger than a screen pixel.
        let pixels_per_point = ui.ctx().pixels_per_point();
        let room = tray.shrink(14.0);
        // Under the layout bar, with some air.
        let room = egui::Rect::from_min_max(room.min + egui::vec2(0.0, 46.0), room.max);
        let scale = ((room.width() - 2.0 * beside) / paper_dots.x)
            .min((room.height() - below) / paper_dots.y)
            .min(1.0 / pixels_per_point);
        let paper_size = paper_dots * scale;
        let paper =
            egui::Rect::from_center_size(room.center() - egui::vec2(0.0, below / 2.0), paper_size);
        let corner = if continuous {
            1
        } else {
            (paper_size.min_elem() * 0.04).clamp(3.0, 10.0) as u8
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
                style::regular((paper_size.min_elem() * 0.12).clamp(13.0, 22.0)),
                style::LIGHT.faint,
            );
        } else {
            // Shrunk on the CPU to whole screen pixels and drawn 1:1 on the pixel grid: sharper
            // than the GPU's filtering.
            let pixels = [
                ((w * scale * pixels_per_point).round() as u32).max(1),
                ((h * scale * pixels_per_point).round() as u32).max(1),
            ];
            let points = egui::vec2(pixels[0] as f32, pixels[1] as f32) / pixels_per_point;
            let corner = paper.center() - points / 2.0;
            let corner = egui::pos2(
                (corner.x * pixels_per_point).round() / pixels_per_point,
                (corner.y * pixels_per_point).round() / pixels_per_point,
            );
            if let Some(preview) = &mut self.preview {
                if preview
                    .texture
                    .as_ref()
                    .is_none_or(|(made, _)| *made != pixels)
                {
                    let small = image::imageops::resize(
                        &preview.image,
                        pixels[0],
                        pixels[1],
                        image::imageops::FilterType::CatmullRom,
                    );
                    let color = egui::ColorImage::from_gray(
                        [small.width() as usize, small.height() as usize],
                        small.as_raw(),
                    );
                    let texture = ui.ctx().load_texture(
                        "quicklabel_preview",
                        color,
                        egui::TextureOptions::NEAREST,
                    );
                    preview.texture = Some((pixels, texture));
                }
                if let Some((_, texture)) = &preview.texture {
                    egui::Image::from_texture((texture.id(), points))
                        .paint_at(ui, egui::Rect::from_min_size(corner, points));
                }
            }
        }

        // The sizes as dimension lines: the tape's width across it, the label's length along
        // it (none for an empty label on free tape: it has no length yet).
        let length_mm = if continuous {
            (label.is_empty() && length == Length::Free)
                .then_some(())
                .map_or_else(|| Some((along / DOTS_PER_MM).round()), |()| None)
        } else {
            Some(f32::from(media.length_mm))
        };
        let width_mm = f32::from(media.width_mm);
        let (horizontal, vertical) = if along_tape {
            (length_mm, Some(width_mm))
        } else {
            (Some(width_mm), length_mm)
        };
        let ink = c.muted;
        if let Some(mm) = horizontal {
            let y = paper.bottom() + 14.0;
            dimension(
                &painter,
                [egui::pos2(paper.left(), y), egui::pos2(paper.right(), y)],
                &format!("{mm} mm"),
                ink,
                c.tray,
            );
        }
        if let Some(mm) = vertical {
            let x = paper.right() + 16.0;
            dimension(
                &painter,
                [egui::pos2(x, paper.top()), egui::pos2(x, paper.bottom())],
                &format!("{mm} mm"),
                ink,
                c.tray,
            );
        }

        // The outcome, as a pill at the tray's foot; a success fades after a few seconds.
        let opacity = self.message_opacity(ui.ctx());
        let pill = match &self.message {
            Some(Ok(text)) if opacity > 0.0 => Some((text.clone(), c.green, c.green_ink, true)),
            Some(Ok(_)) => None,
            Some(Err(text)) => Some((text.clone(), c.alarm, Color32::WHITE, false)),
            None if self.printing => Some((t("Printing…").to_owned(), c.plate, c.text, false)),
            None => None,
        };
        if let Some((text, fill, colour, check)) = pill {
            let (fill, colour) = (fill.gamma_multiply(opacity), colour.gamma_multiply(opacity));
            let galley = painter.layout(text, style::medium(15.0), colour, tray.width() - 60.0);
            let mark = if check { 20.0 } else { 0.0 };
            let size = galley.size() + egui::vec2(28.0 + mark, 12.0);
            let rect = egui::Rect::from_center_size(
                egui::pos2(tray.center().x, tray.bottom() - 12.0 - size.y / 2.0),
                size,
            );
            let shadow = egui::Shadow {
                color: shadow.color.gamma_multiply(opacity),
                ..shadow
            };
            painter.add(shadow.as_shape(rect, CornerRadius::same(16)));
            painter.rect_filled(rect, CornerRadius::same(16), fill);
            if check {
                // Drawn: the font has no check mark.
                let at = egui::pos2(rect.left() + 15.0, rect.center().y);
                painter.line(
                    vec![
                        at + egui::vec2(0.0, 0.5),
                        at + egui::vec2(4.0, 4.5),
                        at + egui::vec2(11.0, -4.5),
                    ],
                    egui::Stroke::new(2.2, colour),
                );
            }
            painter.galley(rect.min + egui::vec2(14.0 + mark, 6.0), galley, colour);
        }
    }

    /// How visible the message is: 1, fading to 0 from [`MESSAGE_SHOWN`] after a success
    /// appeared. An error stays.
    fn message_opacity(&mut self, ctx: &egui::Context) -> f32 {
        let Some(message) = &self.message else {
            self.message_since = None;
            return 1.0;
        };
        if self
            .message_since
            .as_ref()
            .is_none_or(|(seen, _)| seen != message)
        {
            self.message_since = Some((message.clone(), Instant::now()));
        }
        if message.is_err() {
            return 1.0;
        }
        let age = self
            .message_since
            .as_ref()
            .map_or(Duration::ZERO, |(_, since)| since.elapsed());
        let Some(fading) = age.checked_sub(MESSAGE_SHOWN) else {
            ctx.request_repaint_after(MESSAGE_SHOWN - age);
            return 1.0;
        };
        if fading >= MESSAGE_FADE {
            return 0.0;
        }
        ctx.request_repaint();
        1.0 - fading.as_secs_f32() / MESSAGE_FADE.as_secs_f32()
    }

    /// How many labels: − n +, Ctrl+Up / Ctrl+Down.
    fn copies_stepper(&mut self, ui: &mut Ui) {
        let c = style::palette(ui.ctx());
        let hint = t(
            "Number of labels (Ctrl+N to type it, Ctrl+Up / Ctrl+Down). {n} in the text numbers them, {n+12} from 13.",
        );
        egui::Frame::new()
            .fill(c.plate)
            .stroke(egui::Stroke::new(1.0, c.rule))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(4, 4))
            .show(ui, |ui| {
                ui.set_height(40.0);
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if style::step(ui, "+", self.copies < MAX_COPIES)
                        .on_hover_text(hint)
                        .clicked()
                    {
                        self.copies += 1;
                    }
                    ui.allocate_ui_with_layout(
                        egui::vec2(34.0, 40.0),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| {
                            ui.add_space(1.0);
                            if let Some(typed) = &mut self.copies_typed {
                                let field = ui.add(
                                    egui::TextEdit::singleline(typed)
                                        .id(egui::Id::unique(COPIES_FIELD))
                                        .font(style::semibold(19.0))
                                        .align(egui::Align2::CENTER_CENTER)
                                        .char_limit(2)
                                        .frame(egui::Frame::NONE)
                                        .margin(egui::Margin::ZERO)
                                        .desired_width(34.0),
                                );
                                typed.retain(|c| c.is_ascii_digit());
                                if let Ok(copies) = typed.parse::<u16>() {
                                    self.copies = copies.clamp(1, MAX_COPIES);
                                }
                                // Enter or a click elsewhere: done, back to the text.
                                if field.lost_focus() {
                                    self.copies_typed = None;
                                    ui.ctx().memory_mut(|memory| {
                                        memory.request_focus(egui::Id::unique(FIELD));
                                    });
                                }
                            } else if ui
                                .add(
                                    egui::Label::new(
                                        RichText::new(self.copies.to_string())
                                            .font(style::semibold(19.0)),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                                .on_hover_text(hint)
                                .clicked()
                            {
                                self.type_copies(ui.ctx());
                            }
                            ui.add_space(-6.0);
                            ui.label(
                                RichText::new(t("labels"))
                                    .font(style::regular(11.5))
                                    .color(c.muted),
                            );
                        },
                    );
                    if style::step(ui, "−", self.copies > 1)
                        .on_hover_text(hint)
                        .clicked()
                    {
                        self.copies -= 1;
                    }
                });
            });
    }

    /// The typeface: a drop-down of the three.
    fn typeface_menu(&mut self, ui: &mut Ui) {
        let current = self.settings.typeface();
        let mut chosen = current;
        egui::ComboBox::from_id_salt("typeface")
            .selected_text(RichText::new(current.name()).font(style::medium(14.0)))
            .width(78.0)
            .show_ui(ui, |ui| {
                for typeface in Typeface::ALL {
                    ui.selectable_value(
                        &mut chosen,
                        typeface,
                        RichText::new(typeface.name()).font(style::medium(15.0)),
                    );
                }
            })
            .response
            .on_hover_text(t("Typeface of the label"));
        if chosen != current {
            self.settings.typeface = chosen.code().to_owned();
            self.save();
        }
    }

    /// Over the tray: lengthwise or sideways, the cut, and for continuous tape the longest
    /// label.
    fn layout_bar(&mut self, ui: &mut Ui, tray: egui::Rect, media: &'static Media, layout: Layout) {
        let c = style::palette(ui.ctx());
        let bar = egui::Rect::from_min_size(
            tray.min + egui::vec2(14.0, 12.0),
            egui::vec2(tray.width() - 28.0, 26.0),
        );
        let mut bar_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(bar)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let ui = &mut bar_ui;
        ui.spacing_mut().item_spacing.x = 4.0;
        let mut changed = layout;

        // One chip that turns: the way the text runs now.
        let direction = if layout.sideways {
            t("Sideways")
        } else {
            t("Lengthwise")
        };
        if style::direction_chip(ui, direction, layout.sideways, media.is_continuous())
            .on_hover_text(t("Click or Ctrl+R turns the text"))
            .clicked()
        {
            changed.sideways = !layout.sideways;
        }
        if style::chip(ui, t("Fill lines"), layout.fill_lines, None)
            .on_hover_text(t("Every line as large as fits the width (Ctrl+F)"))
            .clicked()
        {
            changed.fill_lines = !layout.fill_lines;
        }

        // On the right, from the right: length (continuous tape only), then cut.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if media.is_continuous() {
                for times in LENGTHS.into_iter().rev() {
                    let name = if times == 0 {
                        t("free").to_owned()
                    } else {
                        format!("{times}×")
                    };
                    let hint = if times == 0 {
                        t("As long as the text needs").to_owned()
                    } else {
                        tf(
                            "At most {length} mm: {times} × the tape's width",
                            &[
                                ("length", &(u32::from(media.width_mm) * u32::from(times))),
                                ("times", &times),
                            ],
                        )
                    };
                    if style::chip(ui, &name, layout.length == times, None)
                        .on_hover_text(hint)
                        .clicked()
                    {
                        changed.length = times;
                    }
                }
                ui.label(
                    RichText::new(t("Length"))
                        .font(style::regular(14.0))
                        .color(c.muted),
                );
                ui.add_space(12.0);
            }

            for (cut, name, hint) in [
                (Cut::Off, t("off"), t("Don't cut: tear off by hand")),
                (Cut::Batch, t("batch"), t("Cut once, after the last label")),
                (Cut::Each, t("each"), t("Cut after every label")),
            ] {
                if style::chip(ui, name, layout.cut == cut, None)
                    .on_hover_text(hint)
                    .clicked()
                {
                    changed.cut = cut;
                }
            }
            ui.label(
                RichText::new(t("Cut"))
                    .font(style::regular(14.0))
                    .color(c.muted),
            );
        });

        if changed != layout {
            self.set_layout(media, changed);
        }
    }

    /// The printer used, as a chip with its status dot; with several, a click opens the list:
    /// automatic (the first ready) or one of them.
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
        let shown = target
            .or(self.chosen.filter(|&index| index < self.printers.len()))
            .unwrap_or(0);
        let link = &self.printers[shown];
        let (text, dot) = describe(&link.host, &link.state(), c);
        if self.printers.len() == 1 {
            style::chip(ui, &text, target.is_some(), Some(dot));
            return;
        }

        let chip = style::menu_chip(ui, &text, target.is_some(), Some(dot))
            .on_hover_text(t("Pick the printer"));
        egui::Popup::menu(&chip).show(|ui| {
            ui.set_min_width(chip.rect.width().max(260.0));
            let automatic =
                RichText::new(t("Automatic: the first that is ready")).font(style::medium(14.0));
            if ui
                .selectable_label(self.chosen.is_none(), automatic)
                .clicked()
            {
                self.chosen = None;
            }
            ui.separator();
            for index in 0..self.printers.len() {
                let link = &self.printers[index];
                let (text, dot) = describe(&link.host, &link.state(), c);
                let picked = self.chosen == Some(index);
                let row = ui
                    .horizontal(|ui| {
                        let (spot, _) =
                            ui.allocate_exact_size(egui::Vec2::splat(10.0), egui::Sense::hover());
                        ui.painter().circle_filled(spot.center(), 4.5, dot);
                        ui.selectable_label(picked, RichText::new(&text).font(style::medium(14.0)))
                    })
                    .inner;
                if row.clicked() {
                    self.chosen = Some(index);
                }
            }
        });
    }

    /// The language switch: one pill EN | NL; a click switches.
    fn language_switch(&mut self, ui: &mut Ui) {
        let names: Vec<&str> = Language::ALL
            .iter()
            .map(|language| language.short())
            .collect();
        let current = Language::ALL
            .iter()
            .position(|language| *language == crate::i18n::language())
            .unwrap_or(0);
        if let Some(next) = style::segmented(ui, &names, current) {
            let language = Language::ALL[next];
            crate::i18n::set_language(language);
            self.settings.language = language.code().to_owned();
            self.save();
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

    /// Remembers the text for the next start.
    fn keep_text(&mut self) {
        if self.settings.last_text != self.text {
            self.settings.last_text.clone_from(&self.text);
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
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
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
                        "Brother QL printers on the network (by address) or on USB. A label goes to the first that is ready with tape in; click one below the label to pick it.",
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
                            let options = self
                                .settings
                                .printer_options
                                .entry(link.host.clone())
                                .or_default();
                            let hint = if link.has_high_resolution()
                                || !matches!(link.state(), State::Online { .. })
                            {
                                t("High resolution: 600 dpi along the tape, sharper and a little slower")
                            } else {
                                t("High resolution: this model doesn't have it, it prints at 300 dpi")
                            };
                            if style::chip(ui, "HQ", options.high_resolution, None)
                                .on_hover_text(hint)
                                .clicked()
                            {
                                options.high_resolution = !options.high_resolution;
                                changed = true;
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
                if self.usb_section(ui, &ctx) {
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

                ui.add_space(8.0);
                if self.search_section(ui, &ctx) {
                    changed = true;
                }

                ui.add_space(14.0);
                if ui
                    .checkbox(
                        &mut self.settings.close_after_print,
                        RichText::new(t("Close the window after printing"))
                            .font(style::medium(16.0)),
                    )
                    .changed()
                {
                    changed = true;
                }

                if cfg!(windows) {
                    ui.add_space(18.0);
                    self.resident_section(ui);
                }
                });
            });

        if changed {
            self.save();
        }
    }

    /// Brother printers plugged in on USB and not added yet, each with Add (looked for every few
    /// seconds while the card is open); whether one was added.
    fn usb_section(&mut self, ui: &mut Ui, ctx: &egui::Context) -> bool {
        let c = style::palette(ui.ctx());
        let stale = self
            .usb_found
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(3));
        if stale {
            let found = ql_raster::usb::find()
                .into_iter()
                .map(|printer| printer.address)
                .collect();
            self.usb_found = Some((Instant::now(), found));
            ctx.request_repaint_after(Duration::from_secs(3));
        }
        let mut add = None;
        for address in self.usb_found.iter().flat_map(|(_, found)| found) {
            if self.settings.printers.contains(address) {
                continue;
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(address).font(style::medium(16.0)));
                ui.label(
                    RichText::new(t("Brother QL on USB"))
                        .font(style::regular(14.0))
                        .color(c.muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(t("Add")).clicked() {
                        add = Some(address.clone());
                    }
                });
            });
        }
        let Some(address) = add else {
            return false;
        };
        self.printers.push(start_link(&address, ctx));
        self.settings.printers.push(address);
        true
    }

    /// The printer search: a button, then the Brother QL printers found with Add; whether one
    /// was added.
    fn search_section(&mut self, ui: &mut Ui, ctx: &egui::Context) -> bool {
        let c = style::palette(ui.ctx());
        let known: Vec<std::net::Ipv4Addr> = self
            .settings
            .printers
            .iter()
            .filter_map(|host| host.parse().ok())
            .collect();

        if let Some(receiver) = &self.search {
            match receiver.try_recv() {
                Ok(found) => {
                    let networks = ql_raster::net::searched_networks(&known);
                    self.searched = Some((networks, found));
                    self.search = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ui.ctx().request_repaint_after(Duration::from_millis(100));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.search = None,
            }
        }

        let searching = self.search.is_some();
        ui.horizontal(|ui| {
            let text = if searching {
                t("Searching…")
            } else {
                t("Search the network")
            };
            if ui
                .add_enabled(!searching, egui::Button::new(text))
                .on_hover_text(t("Looks for Brother QL printers on USB and asks every address of this network (and of the printers set) for its model, about 2 seconds"))
                .clicked()
            {
                let (sender, receiver) = std::sync::mpsc::channel();
                let ctx = ctx.clone();
                let known = known.clone();
                std::thread::spawn(move || {
                    let mut found: Vec<Found> = ql_raster::usb::find()
                        .into_iter()
                        .map(|printer| {
                            // Not over USB when it answers on the network.
                            let model = crate::printer::network_status(&printer.address)
                                .map_or_else(
                                    || ql_raster::usb::query_status(&printer.address),
                                    Ok,
                                )
                                .map_or_else(|_| "Brother QL".to_owned(), |status| {
                                    format!("Brother {}", status.model)
                                });
                            (printer.address, format!("{model} · USB"))
                        })
                        .collect();
                    found.extend(
                        ql_raster::net::discover(Duration::from_millis(1500), &known)
                            .into_iter()
                            .filter(ql_raster::net::Found::is_brother_ql)
                            .map(|device| (device.address.to_string(), device.model)),
                    );
                    let _ = sender.send(found);
                    ctx.request_repaint();
                });
                self.search = Some(receiver);
            }
        });

        let mut added = false;
        if let Some((networks, found)) = &self.searched {
            if found.is_empty() {
                ui.label(
                    RichText::new(tf(
                        "No Brother QL printer on USB or answering on {networks}",
                        &[("networks", &networks.join(", "))],
                    ))
                    .font(style::regular(14.0))
                    .color(c.muted),
                );
            }
            let mut add = None;
            for (host, model) in found {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(host).font(style::medium(16.0)));
                    ui.label(
                        RichText::new(model)
                            .font(style::regular(14.0))
                            .color(c.muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.settings.printers.contains(host) {
                            ui.label(
                                RichText::new(t("added"))
                                    .font(style::regular(14.0))
                                    .color(c.accent),
                            );
                        } else if ui.button(t("Add")).clicked() {
                            add = Some(host.clone());
                        }
                    });
                });
            }
            if let Some(host) = add {
                self.printers.push(start_link(&host, ctx));
                self.settings.printers.push(host);
                added = true;
            }
        }
        added
    }

    /// Resident mode: on or off, and whether the background process runs.
    fn resident_section(&mut self, ui: &mut Ui) {
        let c = style::palette(ui.ctx());
        let hotkey = self.settings.hotkey().to_owned();
        let shown = self.settings.hotkey_shown();
        ui.label(RichText::new(t("Hotkey")).font(style::semibold(19.0)));
        ui.label(
            RichText::new(tf(
                "{hotkey} opens Quick label from anywhere. Windows itself takes a few seconds for it; ready in the background, it opens at once.",
                &[("hotkey", &shown)],
            ))
            .font(style::regular(14.0))
            .color(c.muted),
        );
        ui.add_space(6.0);

        let mut on = self.settings.resident;
        let toggle = ui.checkbox(
            &mut on,
            RichText::new(t("Stay ready in the background (starts with Windows)"))
                .font(style::medium(16.0)),
        );
        if toggle.changed() {
            self.resident_error = switch_resident(on, &hotkey).err();
            self.settings.resident = on;
            self.resident_running = None;
            self.save();
        }

        // Asked every few seconds while the card is open.
        let stale = self
            .resident_running
            .is_none_or(|(_, at)| at.elapsed() > Duration::from_secs(2));
        if stale {
            self.resident_running = Some((resident::is_running(), Instant::now()));
            ui.ctx().request_repaint_after(Duration::from_secs(2));
        }
        let running = self.resident_running.is_some_and(|(running, _)| running);
        let (dot, text) = match (self.settings.resident, running) {
            (true, true) => (c.accent, t("Running in the background")),
            (true, false) => (
                c.alarm,
                t("On, but not running: switch it off and on again"),
            ),
            (false, true) => (c.echo, t("Off, but still running")),
            (false, false) => (c.faint, t("Off: the Start menu shortcut has the key")),
        };
        ui.horizontal(|ui| {
            let (spot, _) = ui.allocate_exact_size(egui::Vec2::splat(10.0), egui::Sense::hover());
            ui.painter().circle_filled(spot.center(), 4.5, dot);
            ui.label(
                RichText::new(text)
                    .font(style::regular(14.0))
                    .color(c.muted),
            );
        });
        if let Some(error) = &self.resident_error {
            ui.label(
                RichText::new(error)
                    .font(style::regular(14.0))
                    .color(c.alarm),
            );
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

/// The label's length and the way the text runs on `media`, for `layout`. Lengthwise is along
/// the label's long side: along continuous tape, along a die-cut label's longer side.
pub(crate) fn shape(media: &Media, layout: Layout) -> (Length, Run) {
    let across = u32::from(media.print_pins);
    let length = if media.is_continuous() {
        match layout.length {
            0 => Length::Free,
            // The cut label `times` the tape's width long, feed included.
            times => {
                let mm = f32::from(media.width_mm) * f32::from(times);
                Length::AtMost(((mm * DOTS_PER_MM) as u32).saturating_sub(2 * FEED_DOTS))
            }
        }
    } else {
        Length::Fixed(u32::from(media.print_length_dots))
    };
    let long_along_tape = match length {
        Length::Fixed(lines) => lines > across,
        _ => true,
    };
    let run = if long_along_tape != layout.sideways {
        Run::AlongTape
    } else {
        Run::AcrossTape
    };
    (length, run)
}

/// Switches resident mode on or off (Windows; elsewhere nothing to do).
fn switch_resident(on: bool, hotkey: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        if on {
            resident::enable(hotkey)
        } else {
            resident::disable(hotkey)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (on, hotkey);
        Ok(())
    }
}

/// A dimension line from `ends[0]` to `ends[1]` (horizontal or vertical) with end ticks and
/// `text` in its middle, on the `background` so the line stops around it; vertical text reads
/// upwards.
fn dimension(
    painter: &egui::Painter,
    ends: [egui::Pos2; 2],
    text: &str,
    ink: Color32,
    background: Color32,
) {
    let stroke = egui::Stroke::new(1.0, ink);
    let vertical = (ends[1].x - ends[0].x).abs() < 0.5;
    let tick = if vertical {
        egui::vec2(4.0, 0.0)
    } else {
        egui::vec2(0.0, 4.0)
    };
    painter.line_segment(ends, stroke);
    for end in ends {
        painter.line_segment([end - tick, end + tick], stroke);
    }

    let galley = painter.layout_no_wrap(text.to_owned(), style::regular(12.5), ink);
    let size = galley.size();
    let middle = egui::pos2((ends[0].x + ends[1].x) / 2.0, (ends[0].y + ends[1].y) / 2.0);
    if vertical {
        painter.rect_filled(
            egui::Rect::from_center_size(middle, egui::vec2(size.y, size.x + 8.0)),
            CornerRadius::ZERO,
            background,
        );
        // Turned a quarter anticlockwise about its top left: placed so its middle is `middle`.
        let at = egui::pos2(middle.x - size.y / 2.0, middle.y + size.x / 2.0);
        painter.add(
            egui::epaint::TextShape::new(at, galley, ink).with_angle(-std::f32::consts::FRAC_PI_2),
        );
    } else {
        painter.rect_filled(
            egui::Rect::from_center_size(middle, egui::vec2(size.x + 8.0, size.y)),
            CornerRadius::ZERO,
            background,
        );
        painter.galley(middle - size / 2.0, galley, ink);
    }
}

/// A printer's chip text and status dot.
fn describe(host: &str, state: &State, c: &style::Palette) -> (String, Color32) {
    match state {
        State::Online {
            status,
            model,
            name,
        } => (
            {
                // Its own name first, then the model without "Brother": both, when it has a
                // name.
                let model = model.as_deref().unwrap_or(host);
                let printer = match name {
                    Some(name) => format!("{name} · {}", model.trim_start_matches("Brother ")),
                    None => model.to_owned(),
                };
                format!("{printer} · {}", status.media_description())
            },
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
                    ui.label(
                        RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                            .font(style::regular(12.5))
                            .color(c.faint),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        self.language_switch(ui);
                        ui.add_space(6.0);
                        self.theme_switch(ui);
                        ui.add_space(6.0);
                        if style::chip(ui, t("Settings"), self.show_printers, None)
                            .on_hover_text(t("Printers and the hotkey"))
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
                    let label = match (self.printers.is_empty(), self.copies) {
                        (true, 1) => t("Save as PNG").to_owned(),
                        (true, copies) => tf("Save {copies} as PNG", &[("copies", &copies)]),
                        (false, 1) => t("Print").to_owned(),
                        (false, copies) => tf("Print {copies}", &[("copies", &copies)]),
                    };
                    let ready = !TextLabel::new(&self.text).is_empty()
                        && !self.printing
                        && (self.printers.is_empty() || self.target().is_some());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if style::primary(ui, &label, "Ctrl + Enter", ready).clicked() {
                            self.print();
                        }
                        ui.add_space(4.0);
                        self.copies_stepper(ui);
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
                            // The last text, selected: typing replaces it, Ctrl+Enter prints it.
                            response.request_focus();
                            let mut state =
                                egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                            state.cursor.set_char_range(Some(CCursorRange::two(
                                CCursor::new(0),
                                CCursor::new(self.text.chars().count()),
                            )));
                            state.store(ui.ctx(), id);
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
                    style::shortcut(ui, &["Ctrl", "R"], t("turn"));
                    ui.label(
                        RichText::new("{n}")
                            .font(style::medium(14.0))
                            .color(c.faint),
                    );
                    ui.label(
                        RichText::new(t("number"))
                            .font(style::regular(14.0))
                            .color(c.muted),
                    )
                    .on_hover_text(t(
                        "{n} becomes 1, 2, 3 … over the labels; {n+12} starts at 13",
                    ));
                    ui.add_space(10.0);
                    style::shortcut(ui, &["Esc"], t("close"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.typeface_menu(ui);
                    });
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

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.keep_text();
    }
}
