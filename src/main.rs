//! Quick label: type a few lines, Ctrl+Enter prints them on a Brother QL label printer.
//!
//! How to use: start `quicklabel` (on Windows under a hotkey: `install.ps1` puts it under
//! Ctrl+Alt+L). Type; Enter starts a new line; `**bold**`, `*italic*`; Ctrl+D inserts the date,
//! Ctrl+T the time; Ctrl+R turns the text; Ctrl+Up/Down sets the number of labels, `{n}` numbers
//! them (`{n+12}` from 13); Ctrl+Enter prints and closes; Esc closes. The first time, add the printer's
//! address under Printers.
//!
//! Working principle: one small window. The text is laid out for the tape the printer reports
//! ([`chopshop_label::TextLabel`]): along continuous tape as long as the text needs, a die-cut
//! label filled in the direction with the larger letters. Printers and language are kept in
//! `%APPDATA%\iotta\quicklabel.json` (elsewhere `~/.config/iotta/quicklabel.json`; the
//! `QUICKLABEL_SETTINGS` variable points elsewhere). `--text <text>` starts with a text,
//! `--screenshot <png>` saves the window after a moment and quits (docs); `--dark` or `--light`
//! overrides Windows' theme. `--resident` is resident mode's background process;
//! `--find-printers` lists what answers the printer search
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

/// Lists what answers on the network, to stdout (to check the search).
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
        println!("{}  {}", device.address, device.model);
    }
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
