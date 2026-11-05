//! Quick label: type a few lines, Ctrl+Enter prints them on a Brother QL label printer.
//!
//! How to use: start `quicklabel` (on Windows under a hotkey: `install.ps1` puts it under
//! Ctrl+Alt+L). Type; Enter starts a new line; `**bold**`, `*italic*`; Ctrl+D inserts the date,
//! Ctrl+T the time; Ctrl+Enter prints and closes; Esc closes. The first time, add the printer's
//! address under Printers.
//!
//! Working principle: one small window. The text is laid out for the tape the printer reports
//! ([`chopshop_label::TextLabel`]): along continuous tape as long as the text needs, a die-cut
//! label filled in the direction with the larger letters. Printers and language are kept in
//! `%APPDATA%\iotta\quicklabel.json` (elsewhere `~/.config/iotta/quicklabel.json`; the
//! `QUICKLABEL_SETTINGS` variable points elsewhere). `--text <text>` starts with a text,
//! `--screenshot <png>` saves the window after a moment and quits (docs).

// A GUI program on Windows: no console window when the hotkey starts it.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod printer;
mod settings;
mod style;

/// The Dutch texts.
const DUTCH: &str = include_str!("../i18n/nl.toml");

fn main() -> eframe::Result {
    let mut text = None;
    let mut screenshot = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--text" => text = args.next(),
            "--screenshot" => screenshot = args.next().map(std::path::PathBuf::from),
            _ => {}
        }
    }

    chopshop_i18n::install_dutch(DUTCH);
    app::run(text, screenshot)
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
