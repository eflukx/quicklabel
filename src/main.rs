//! Quick label: type a few lines, Ctrl+Enter prints them on a Brother QL label printer.
//!
//! How to use: start `quicklabel` (`install.ps1` puts it in the Start menu; Settings: resident
//! mode puts it under Ctrl+Alt+L). Type; Enter starts a new line; `**bold**`, `*italic*`; Ctrl+D inserts the date,
//! Ctrl+T the time; Ctrl+R turns the text; Ctrl+Up/Down sets the number of labels (Ctrl+N types it), `{n}` numbers
//! them (`{n+12}` from 13); Ctrl+Enter prints; Esc closes. The first time, add the printer's
//! address under Printers.
//!
//! Working principle: one small window. The text is laid out for the tape the printer reports
//! ([`chopshop_label::TextLabel`]): along continuous tape as long as the text needs, a die-cut
//! label filled in the direction with the larger letters. Printers and language are kept in
//! `%APPDATA%\iotta\quicklabel.json` (elsewhere `~/.config/iotta/quicklabel.json`; the
//! `QUICKLABEL_SETTINGS` variable points elsewhere). `--text <text>` starts with a text,
//! `--screenshot <png>` saves the window after a moment and quits (docs); `--dark` or `--light`
//! overrides Windows' theme. `--resident` is resident mode's background process;
//! `--find-printers` lists what answers the printer search and the printers on USB;
//! `--print <address> <text>` prints once, without the window (`usb:<serial>` for USB)
//! ([`resident`]).

// A GUI program on Windows: no console window when the hotkey starts it.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod markdown;
mod numbering;
mod printer;
mod resident;
mod settings;
mod style;

/// The Dutch texts.
const DUTCH: &str = include_str!("../i18n/nl.toml");

fn main() -> eframe::Result {
    let mut text = None;
    let mut screenshot = None;
    let mut theme = None;
    let mut resident = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--text" => text = args.next(),
            "--screenshot" => screenshot = args.next().map(std::path::PathBuf::from),
            "--dark" => theme = Some(egui::Theme::Dark),
            "--light" => theme = Some(egui::Theme::Light),
            "--resident" => resident = true,
            "--find-printers" => {
                find_printers();
                return Ok(());
            }
            "--abort" => {
                let address = args.next().unwrap_or_default();
                match chopshop_printer::usb::abort(&address) {
                    Ok(()) => println!("{address}: job stopped"),
                    Err(error) => eprintln!("{address}: {error}"),
                }
                return Ok(());
            }
            "--print" => {
                let (address, text) = (args.next().unwrap_or_default(), args.next());
                match print_once(&address, &text.unwrap_or_default()) {
                    Ok(done) => println!("{done}"),
                    Err(error) => {
                        eprintln!("{address}: {error}");
                        std::process::exit(1);
                    }
                }
                return Ok(());
            }
            _ => {}
        }
    }

    chopshop_i18n::install_dutch(DUTCH);
    if resident {
        run_resident();
        return Ok(());
    }

    // One window: a second start brings the open one forward.
    let window = if screenshot.is_some() {
        None
    } else {
        match resident::claim(resident::WINDOW_PORT) {
            Some(listener) => Some(listener),
            None if resident::send(resident::WINDOW_PORT, "show") => return Ok(()),
            None => None,
        }
    };
    app::run(text, screenshot, theme, window)
}

/// Lists what answers on the network and the printers on USB, to stdout (to check the search).
fn find_printers() {
    let own = chopshop_printer::net::local_address();
    println!("this PC: {own:?}");
    let known: Vec<std::net::Ipv4Addr> = settings::Settings::load()
        .printers
        .iter()
        .filter_map(|host| host.parse().ok())
        .collect();
    println!(
        "searching: {}",
        chopshop_printer::net::searched_networks(&known).join(", ")
    );
    for device in chopshop_printer::net::discover(std::time::Duration::from_millis(1500), &known) {
        let host = device.address.to_string();
        let errors = chopshop_printer::net::query_status(&host)
            .map(|status| format!("{:?}", status.error_names()))
            .unwrap_or_else(|error| error.to_string());
        println!("{host}  {}  errors: {errors}", device.model);
    }
    for printer in chopshop_printer::usb::find() {
        let status = chopshop_printer::usb::query_status(&printer.address);
        match status {
            Ok(status) => println!(
                "{}  {} · {} · errors: {:?}",
                printer.address,
                status.model,
                status.media_description(),
                status.error_names()
            ),
            Err(error) => println!("{}  {error}", printer.address),
        }
    }
}

/// Prints `text` once on the printer at `address` (network or `usb:<serial>`), with the
/// settings of the window: layout of the loaded tape, typeface, HQ.
fn print_once(address: &str, text: &str) -> Result<String, String> {
    use chopshop_printer::job::{self, Resolution};
    use chopshop_printer::{net, raster, usb};

    let settings = settings::Settings::load();
    let status = if usb::is_usb(address) {
        usb::query_status(address)
    } else {
        net::query_status(address)
    }
    .map_err(|error| error.to_string())?;
    let dump = std::env::var_os("QUICKLABEL_JOB");
    if !status.is_ready() && dump.is_none() {
        return Err(status.error_names().join(", "));
    }
    let media = status
        .loaded_media()
        .ok_or_else(|| "no known tape loaded".to_owned())?;
    let layout = settings.layouts.get(media.id).copied().unwrap_or_default();
    let (length, run) = app::shape(media, layout);
    let resolution = if settings.printer(address).high_resolution && status.model.high_resolution()
    {
        Resolution::High
    } else {
        Resolution::Normal
    };

    let label = chopshop_label::TextLabel::new(&numbering::fill(text, 0))
        .fill_lines(layout.fill_lines)
        .typeface(settings.typeface());
    if label.is_empty() {
        return Err("no text".to_owned());
    }
    let across = u32::from(media.print_pins);
    let bitmap = match resolution {
        Resolution::Normal => label.render(across, length, run),
        Resolution::High => label.render_fine(across, length, run),
    };
    // QUICKLABEL_PNG: the raster as an image instead of printing (to check a label).
    if let Some(path) = std::env::var_os("QUICKLABEL_PNG") {
        bitmap
            .to_gray_image()
            .save(&path)
            .map_err(|error| error.to_string())?;
        return Ok(format!(
            "saved {} ({} × {}, {resolution:?}, {layout:?})",
            std::path::Path::new(&path).display(),
            bitmap.width(),
            bitmap.height()
        ));
    }
    let lines =
        raster::raster_lines_at(&bitmap, media, resolution).map_err(|error| error.to_string())?;
    let compression = if status.model.compression() {
        job::Compression::Tiff
    } else {
        job::Compression::None
    };
    let bytes = job::pages_bytes(&[&lines], media, layout.cut.job(), resolution, compression);
    // QUICKLABEL_JOB: the job's bytes to a file instead of the printer.
    if let Some(path) = dump {
        std::fs::write(&path, &bytes).map_err(|error| error.to_string())?;
        return Ok(format!(
            "{} bytes to {}",
            bytes.len(),
            path.to_string_lossy()
        ));
    }
    if usb::is_usb(address) {
        usb::send_job(address, &bytes)
    } else {
        net::send_job(address, &bytes)
    }
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "printed on {address} ({}): {media}, {resolution:?}",
        status.model
    ))
}

/// The background process of resident mode; why it stopped goes to `resident.log` next to
/// the settings.
fn run_resident() {
    #[cfg(windows)]
    {
        let settings = settings::Settings::load();
        chopshop_i18n::set_language(if settings.language.is_empty() {
            settings::system_language()
        } else {
            chopshop_i18n::Language::from_code(&settings.language)
        });
        if let Err(error) = resident::run(settings.hotkey()) {
            let log = settings::Settings::path().with_file_name("resident.log");
            let _ = std::fs::write(log, format!("{error}\n"));
        }
    }
    #[cfg(not(windows))]
    eprintln!("resident mode is for Windows");
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::DUTCH;

    #[test]
    fn every_text_has_a_dutch_one() {
        let missing = chopshop_i18n::check::missing(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            DUTCH,
        );
        assert!(
            missing.is_empty(),
            "not in i18n/nl.toml:\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn dutch_keeps_the_placeholders() {
        let wrong = chopshop_i18n::check::placeholder_mismatches(DUTCH);
        assert!(wrong.is_empty(), "other placeholders: {wrong:?}");
    }
}
