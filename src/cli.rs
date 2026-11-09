//! Quick label from the command line: print without the window, list the printers.
//!
//! How to use: `quicklabel "Rack 4" "**do not move**"` prints two lines on the first printer
//! that is ready; `-p 2` on printer 2 of `quicklabel --list-printers`; `--sideways`,
//! `--fill-lines`, `--cut`, `--length`, `-n`, `--font`, `--hq` as the chips in the window.
//! `quicklabel --help` lists everything.
//!
//! Working principle: the options decide. A text or `--list-printers` works without the
//! window; no options, or only the window's (`--text`, `--dark`, `--light`), open it. What
//! isn't given comes from the settings, as the window would print it: the layout kept for the
//! loaded tape, the typeface, HQ per printer; the command line changes no setting. A USB
//! printer that also answers on the network (the same serial) isn't asked over USB: that holds
//! the printer to USB for a few seconds, and network jobs wait.

use std::io::Read;
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use ql_raster::job::{self, Resolution};
use ql_raster::label::Typeface;
use ql_raster::{Media, Status, net, raster, usb};

use crate::settings::{Cut, Layout, Settings};
use crate::{app, numbering};

const EXAMPLES: &str = "\
Examples:
  quicklabel \"Rack 4\" \"**do not move**\"       two lines, on the first printer that is ready
  quicklabel -p 2 -n 10 \"Box {N}\"               ten labels, Box 1 to Box 10, on printer 2
  quicklabel --sideways --length 2 --cut batch \"Spare parts\"
  quicklabel --font serif --no-hq \"Archive\\n2026\"
  echo Hello | quicklabel -                      the text from stdin
  quicklabel --png label.png \"Test\"             the label as an image, not printed
  quicklabel --list-printers                     the printers and their numbers for -p

Text: every argument is a line; \\n in an argument starts a new line too. **bold**, *italic*;
{N} is the label's number (with -n), {N+12} counts from 13, {N+007} keeps the zeros.
Options not given come from Quick label's settings (the layout kept for the loaded tape).
Exit code: 0 printed, 1 not printed, 2 wrong options.";

/// Quick label: type a few lines, print them on a Brother QL label printer.
///
/// Without a text the window opens; with a text (or --list-printers) it prints from the
/// command line, without the window.
// `{N}` in the help texts: clap reads `{n}` as a line break; `parse` shows it as `{n}`.
#[derive(Debug, Parser)]
#[command(name = "quicklabel", version, after_help = EXAMPLES)]
pub struct Args {
    /// The label's text, one argument per line ("-": read it from stdin).
    #[arg(value_name = "TEXT")]
    pub text: Vec<String>,

    /// The printers set in Quick label, numbered for -p, with their status.
    #[arg(short = 'l', long, help_heading = "Printers")]
    pub list_printers: bool,

    /// With --list-printers: also look for printers on the network and USB not set yet.
    #[arg(long, requires = "list_printers", help_heading = "Printers")]
    pub search: bool,

    /// The printer: its number from --list-printers, or an address (192.168.1.57,
    /// usb:000H6Z560817). Default: the first that is ready.
    #[arg(short, long, value_name = "N|ADDRESS", help_heading = "Printing")]
    pub printer: Option<String>,

    /// How many labels (one job); {N} in the text numbers them.
    #[arg(short = 'n', long, value_name = "N", default_value_t = 1,
          value_parser = clap::value_parser!(u16).range(1..=999), help_heading = "Printing")]
    pub copies: u16,

    /// The text along the label's short side (Sideways in the window).
    #[arg(long, overrides_with = "no_sideways", help_heading = "Layout")]
    pub sideways: bool,

    /// The text along the label's long side.
    #[arg(long, overrides_with = "sideways", help_heading = "Layout")]
    pub no_sideways: bool,

    /// Every line as large as fills the width (Fill lines in the window).
    #[arg(long, overrides_with = "no_fill_lines", help_heading = "Layout")]
    pub fill_lines: bool,

    /// All lines in one size.
    #[arg(long, overrides_with = "fill_lines", help_heading = "Layout")]
    pub no_fill_lines: bool,

    /// Where the cutter cuts: after each label, once after the batch, or not.
    #[arg(long, value_enum, help_heading = "Layout")]
    pub cut: Option<CutArg>,

    /// Continuous tape: the label at most this many times the tape's width long (1 to 10),
    /// or free (as long as the text needs).
    #[arg(long, value_name = "free|1|2|3", value_parser = parse_length, help_heading = "Layout")]
    pub length: Option<u8>,

    /// The typeface.
    #[arg(long, value_enum, help_heading = "Layout")]
    pub font: Option<FontArg>,

    /// High resolution (600 dpi along the tape), when the model has it.
    #[arg(long, overrides_with = "no_hq", help_heading = "Printing")]
    pub hq: bool,

    /// Normal resolution (300 dpi).
    #[arg(long, overrides_with = "hq", help_heading = "Printing")]
    pub no_hq: bool,

    /// Save the label(s) as PNG instead of printing (label.png; label_2.png, … with -n).
    #[arg(long, value_name = "FILE", help_heading = "Printing")]
    pub png: Option<PathBuf>,

    /// Open the window with this text.
    #[arg(long = "text", value_name = "TEXT", help_heading = "Window")]
    pub window_text: Option<String>,

    /// The window dark, whatever Windows' theme.
    #[arg(long, conflicts_with = "light", help_heading = "Window")]
    pub dark: bool,

    /// The window light.
    #[arg(long, help_heading = "Window")]
    pub light: bool,

    /// Save the window as PNG after a moment and quit (for the docs).
    #[arg(long, value_name = "PNG", hide = true)]
    pub screenshot: Option<PathBuf>,

    /// Resident mode's background process (holds the hotkey).
    #[arg(long, hide = true)]
    pub resident: bool,

    /// Stops a job a USB printer hangs on.
    #[arg(long, value_name = "ADDRESS", hide = true)]
    pub abort: Option<String>,

    /// The old --print ADDRESS TEXT (now -p ADDRESS TEXT).
    #[arg(long, value_name = "ADDRESS", hide = true)]
    pub print: Option<String>,
}

/// The arguments of this run. Help and version are printed here (exit 0), wrong options
/// exit with 2.
#[must_use]
pub fn parse() -> Args {
    match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            use clap::error::ErrorKind;
            let text = error.render().to_string().replace("{N", "{n");
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                print!("{text}");
                std::process::exit(0);
            }
            eprint!("{text}");
            std::process::exit(2);
        }
    }
}

/// Where the cutter cuts.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CutArg {
    /// After every label.
    Each,
    /// Once after the last label.
    Batch,
    /// Not at all.
    Off,
}

/// A typeface.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FontArg {
    /// Barlow Semi Condensed: narrow (the default).
    Barlow,
    /// Roboto: a plain, wider sans.
    Roboto,
    /// Source Serif 4.
    Serif,
}

fn parse_length(value: &str) -> Result<u8, String> {
    match value {
        "free" | "0" => Ok(0),
        _ => value
            .parse::<u8>()
            .ok()
            .filter(|times| (1..=10).contains(times))
            .ok_or_else(|| "free or 1 to 10".to_owned()),
    }
}

impl Args {
    /// Whether this is a command-line run (no window).
    #[must_use]
    pub fn is_command_line(&self) -> bool {
        !self.text.is_empty() || self.list_printers || self.abort.is_some()
    }

    /// The print options given without a text, if any (they'd do nothing).
    #[must_use]
    pub fn stray_print_options(&self) -> Option<&'static str> {
        let given = [
            (self.printer.is_some() || self.print.is_some(), "--printer"),
            (self.copies != 1, "--copies"),
            (self.sideways || self.no_sideways, "--sideways"),
            (self.fill_lines || self.no_fill_lines, "--fill-lines"),
            (self.cut.is_some(), "--cut"),
            (self.length.is_some(), "--length"),
            (self.font.is_some(), "--font"),
            (self.hq || self.no_hq, "--hq"),
            (self.png.is_some(), "--png"),
        ];
        given
            .into_iter()
            .find(|(set, _)| *set)
            .map(|(_, name)| name)
    }
}

/// Runs a command-line invocation; the exit code.
pub fn run(args: &Args) -> i32 {
    if let Some(address) = &args.abort {
        return match usb::abort(address) {
            Ok(()) => {
                println!("{address}: job stopped");
                0
            }
            Err(error) => {
                eprintln!("{address}: {error}");
                1
            }
        };
    }
    if args.list_printers {
        list_printers(args.search);
        return 0;
    }
    match print(args) {
        Ok(done) => {
            println!("{done}");
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

/// A printer set in Quick label and what it answered.
struct Answer {
    address: String,
    status: Result<Status, String>,
    /// Its name and model (`LabelPrinter_2 · QL-710W`).
    printer: String,
    /// The number of the network printer it is the same as, for a USB one.
    same_as: Option<usize>,
}

/// Asks the printers set, network first: a USB printer that answers on the network too isn't
/// asked over USB (its status is the network's).
fn ask_printers(addresses: &[String]) -> Vec<Answer> {
    let mut answers: Vec<Option<Answer>> = addresses.iter().map(|_| None).collect();
    let mut serials = Vec::new();
    for (index, address) in addresses.iter().enumerate() {
        if usb::is_usb(address) {
            continue;
        }
        let status = net::query_status(address).map_err(|error| error.to_string());
        let mut printer = net::query_model_name(address)
            .map(|model| model.trim_start_matches("Brother ").to_owned())
            .unwrap_or_default();
        if let Ok(status) = &status {
            if let Ok(Some(name)) = net::query_name(address) {
                printer = format!("{name} · {printer}");
            }
            if let Ok(Some(serial)) = net::query_serial(address) {
                serials.push((index, serial, status.clone()));
            }
        }
        answers[index] = Some(Answer {
            address: address.clone(),
            status,
            printer,
            same_as: None,
        });
    }
    for (index, address) in addresses.iter().enumerate() {
        if !usb::is_usb(address) {
            continue;
        }
        let twin = serials
            .iter()
            .find(|(_, serial, _)| usb::has_serial(address, serial));
        let (status, same_as) = match twin {
            // Unplugged, whatever the network says (Windows knows, the printer isn't asked).
            _ if !usb::is_present(address) => (Err("not plugged in".to_owned()), None),
            Some((network, _, status)) => (Ok(status.clone()), Some(*network)),
            None => (
                usb::query_status(address).map_err(|error| error.to_string()),
                None,
            ),
        };
        let printer = status
            .as_ref()
            .map(|status| format!("{} · USB", status.model))
            .unwrap_or_else(|_| "USB".to_owned());
        answers[index] = Some(Answer {
            address: address.clone(),
            status,
            printer,
            same_as,
        });
    }
    answers.into_iter().flatten().collect()
}

fn status_words(status: &Result<Status, String>) -> String {
    match status {
        Ok(status) if status.is_ready() => "ready".to_owned(),
        Ok(status) => status.error_names().join(", "),
        Err(error) => format!("doesn't answer ({error})"),
    }
}

fn list_printers(search: bool) {
    let settings = Settings::load();
    if settings.printers.is_empty() {
        println!("No printers set (add them in the window's Settings, or see --search).");
    }
    let answers = ask_printers(&settings.printers);
    let width = answers
        .iter()
        .map(|answer| answer.address.chars().count())
        .max()
        .unwrap_or(0);
    let printer_width = answers
        .iter()
        .map(|answer| answer.printer.chars().count())
        .max()
        .unwrap_or(0);
    for (index, answer) in answers.iter().enumerate() {
        let tape = answer
            .status
            .as_ref()
            .map(Status::media_description)
            .unwrap_or_default();
        let same = answer
            .same_as
            .map(|network| format!(" (the same printer as {})", network + 1))
            .unwrap_or_default();
        println!(
            "{:>2}  {:<width$}  {:<printer_width$}  {:<18}  {}{same}",
            index + 1,
            answer.address,
            answer.printer,
            tape,
            status_words(&answer.status),
        );
    }
    if !search {
        return;
    }

    println!("\nNot set yet:");
    let known: Vec<std::net::Ipv4Addr> = settings
        .printers
        .iter()
        .filter_map(|host| host.parse().ok())
        .collect();
    let mut found = 0;
    for device in net::discover(std::time::Duration::from_millis(1500), &known) {
        let address = device.address.to_string();
        if device.is_brother_ql() && !settings.printers.contains(&address) {
            match &device.name {
                Some(name) => println!("    {address}  {name} · {}", device.model),
                None => println!("    {address}  {}", device.model),
            }
            found += 1;
        }
    }
    for printer in usb::find() {
        if !settings.printers.contains(&printer.address) {
            println!("    {}  Brother QL on USB", printer.address);
            found += 1;
        }
    }
    if found == 0 {
        println!(
            "    nothing (searched {})",
            net::searched_networks(&known).join(", ")
        );
    }
}

/// The text from the arguments: one per line, `\n` a new line too, `-` stdin.
fn text(args: &Args) -> Result<String, String> {
    let mut lines = Vec::new();
    for arg in &args.text {
        if arg == "-" {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .map_err(|error| format!("stdin: {error}"))?;
            lines.push(input.trim_end().to_owned());
        } else {
            lines.push(arg.replace("\\n", "\n"));
        }
    }
    Ok(lines.join("\n"))
}

/// The printer to print on, with its status: `-p` (number or address), else the first that
/// is ready.
fn choose(args: &Args, settings: &Settings) -> Result<(String, Status), String> {
    let chosen = args.printer.as_ref().or(args.print.as_ref());
    if let Some(chosen) = chosen {
        let address = match chosen.parse::<usize>() {
            Ok(number) => settings
                .printers
                .get(number.wrapping_sub(1))
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "no printer {number}: {} set (see --list-printers)",
                        settings.printers.len()
                    )
                })?,
            Err(_) => chosen.clone(),
        };
        let mut answers = ask_printers(std::slice::from_ref(&address));
        let answer = answers.pop().expect("one asked");
        return match answer.status {
            Ok(status) if status.is_ready() => Ok((address, status)),
            status => Err(format!("{address}: {}", status_words(&status))),
        };
    }

    if settings.printers.is_empty() {
        return Err("no printers set: add one in the window's Settings, or use -p ADDRESS".into());
    }
    let answers = ask_printers(&settings.printers);
    // A USB printer that is also on the network: printed there (it was asked there).
    let ready = answers.iter().find(|answer| {
        answer.same_as.is_none() && matches!(&answer.status, Ok(status) if status.is_ready())
    });
    match ready {
        Some(answer) => Ok((
            answer.address.clone(),
            answer.status.clone().expect("ready"),
        )),
        None => Err(format!(
            "no printer ready: {}",
            answers
                .iter()
                .map(|answer| format!("{} {}", answer.address, status_words(&answer.status)))
                .collect::<Vec<_>>()
                .join("; ")
        )),
    }
}

/// The layout: the one kept for `media`'s tape, with the options given.
fn layout(args: &Args, settings: &Settings, media: &Media) -> Layout {
    let mut layout = settings.layouts.get(media.id).copied().unwrap_or_default();
    if args.sideways || args.no_sideways {
        layout.sideways = args.sideways;
    }
    if args.fill_lines || args.no_fill_lines {
        layout.fill_lines = args.fill_lines;
    }
    if let Some(cut) = args.cut {
        layout.cut = match cut {
            CutArg::Each => Cut::Each,
            CutArg::Batch => Cut::Batch,
            CutArg::Off => Cut::Off,
        };
    }
    if let Some(length) = args.length {
        layout.length = length;
    }
    layout
}

fn print(args: &Args) -> Result<String, String> {
    let settings = Settings::load();
    let text = text(args)?;
    if ql_raster::label::TextLabel::new(&text).is_empty() {
        return Err("no text".to_owned());
    }
    let typeface = match args.font {
        Some(FontArg::Barlow) => Typeface::Barlow,
        Some(FontArg::Roboto) => Typeface::Roboto,
        Some(FontArg::Serif) => Typeface::Serif,
        None => settings.typeface(),
    };

    // A PNG: for the printer's tape; without one ready, 62 mm, as the window does.
    let target = match choose(args, &settings) {
        Ok(target) => Some(target),
        Err(_) if args.png.is_some() && args.printer.is_none() => None,
        Err(error) => return Err(error),
    };
    let media = match &target {
        Some((address, status)) => status
            .loaded_media()
            .ok_or_else(|| format!("{address}: no known tape loaded"))?,
        None => Media::by_id("62").expect("62 mm is in the media table"),
    };
    let layout = layout(args, &settings, media);
    let (length, run) = app::shape(media, layout);
    let high = match (args.hq, args.no_hq, &target) {
        (_, true, _) => false,
        (true, _, Some((_, status))) => status.model.high_resolution(),
        (_, _, Some((address, status))) => {
            settings.printer(address).high_resolution && status.model.high_resolution()
        }
        (_, _, None) => false,
    };
    let resolution = if high {
        Resolution::High
    } else {
        Resolution::Normal
    };

    let across = u32::from(media.print_pins);
    let labels: Vec<_> = (0..args.copies)
        .map(|index| {
            let label = ql_raster::label::TextLabel::new(&numbering::fill(&text, index))
                .fill_lines(layout.fill_lines)
                .typeface(typeface);
            match resolution {
                Resolution::Normal => label.render(across, length, run),
                Resolution::High => label.render_fine(across, length, run),
            }
        })
        .collect();

    if let Some(path) = &args.png {
        let mut saved = Vec::new();
        for (index, bitmap) in labels.iter().enumerate() {
            let file = if index == 0 {
                path.clone()
            } else {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                path.with_file_name(format!("{stem}_{}.png", index + 1))
            };
            bitmap
                .to_gray_image()
                .save(&file)
                .map_err(|error| format!("{}: {error}", file.display()))?;
            saved.push(file.display().to_string());
        }
        return Ok(format!("saved {} ({media})", saved.join(", ")));
    }

    let (address, status) = target.expect("chosen when printing");
    let pages = labels
        .iter()
        .map(|bitmap| raster::raster_lines_at(bitmap, media, resolution))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let pages: Vec<&[_]> = pages.iter().map(Vec::as_slice).collect();
    let compression = if status.model.compression() {
        job::Compression::Tiff
    } else {
        job::Compression::None
    };
    let bytes = job::pages_bytes(&pages, media, layout.cut.job(), resolution, compression);
    // QUICKLABEL_JOB: the job's bytes to a file instead of the printer.
    if let Some(path) = std::env::var_os("QUICKLABEL_JOB") {
        std::fs::write(&path, &bytes).map_err(|error| error.to_string())?;
        return Ok(format!(
            "{} bytes to {}",
            bytes.len(),
            path.to_string_lossy()
        ));
    }
    ql_raster::Printer::new(&address)
        .send_job(&bytes)
        .map_err(|error| format!("{address}: {error}"))?;

    let copies = if args.copies > 1 {
        format!("{} labels", args.copies)
    } else {
        "printed".to_owned()
    };
    let hq = if high { ", HQ" } else { "" };
    Ok(format!(
        "{copies} on {address} ({}): {media}{hq}",
        status.model
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &[&str]) -> Args {
        Args::try_parse_from(std::iter::once("quicklabel").chain(line.iter().copied()))
            .expect("valid")
    }

    #[test]
    fn the_options_decide_window_or_command_line() {
        assert!(!args(&[]).is_command_line());
        assert!(!args(&["--dark", "--text", "Hi"]).is_command_line());
        assert!(args(&["Hi"]).is_command_line());
        assert!(args(&["-l"]).is_command_line());
        assert_eq!(args(&["--cut", "off"]).stray_print_options(), Some("--cut"));
        assert_eq!(args(&["Hi"]).stray_print_options(), None);
    }

    #[test]
    fn options_set_the_layout() {
        let parsed = args(&[
            "--sideways",
            "--no-fill-lines",
            "--cut",
            "batch",
            "--length",
            "free",
            "-n",
            "3",
            "Box {n}",
        ]);
        let media = Media::by_id("62").expect("known");
        let layout = layout(&parsed, &Settings::default(), media);
        assert!(layout.sideways);
        assert!(!layout.fill_lines);
        assert_eq!(layout.cut, Cut::Batch);
        assert_eq!(layout.length, 0);
        assert_eq!(parsed.copies, 3);
        assert!(Args::try_parse_from(["quicklabel", "--length", "11", "x"]).is_err());
        assert!(Args::try_parse_from(["quicklabel", "-n", "0", "x"]).is_err());
    }

    #[test]
    fn lines_from_arguments_and_backslash_n() {
        let parsed = args(&["Rack 4", "a\\nb"]);
        assert_eq!(text(&parsed).expect("no stdin"), "Rack 4\na\nb");
    }
}
