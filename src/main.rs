//! Quick label: type a few lines, Ctrl+Enter prints them on a Brother QL label printer.
//!
//! How to use: start `quicklabel` (`install.ps1` puts it in the Start menu; Settings: resident
//! mode puts it under Ctrl+Alt+L). Type; Enter starts a new line; `**bold**`, `*italic*`; Ctrl+D inserts the date,
//! Ctrl+T the time; Ctrl+R turns the text; Ctrl+Up/Down sets the number of labels (Ctrl+N types it), `{n}` numbers
//! them (`{n+12}` from 13); Ctrl+Enter prints; Esc closes. The first time, add the printer's
//! address under Printers. From the command line: `quicklabel "text"` prints without the
//! window, `quicklabel --help` lists the options ([`cli`]).
//!
//! Working principle: one small window. The text is laid out for the tape the printer reports
//! ([`ql_raster::label::TextLabel`]): along continuous tape as long as the text needs, a die-cut
//! label filled in the direction with the larger letters. Printers and language are kept in
//! `%APPDATA%\iotta\quicklabel.json` (elsewhere `~/.config/iotta/quicklabel.json`; the
//! `QUICKLABEL_SETTINGS` variable points elsewhere). The options decide between window and
//! command line ([`cli::Args::is_command_line`]); `--text <text>` opens the window with a text,
//! `--dark` or `--light` overrides Windows' theme. Windows starts the exe without a console (a
//! window program); a command-line run attaches to the console it was started from.

// A GUI program on Windows: no console window when the hotkey starts it.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod cli;
mod i18n;
mod markdown;
mod numbering;
mod printer;
mod resident;
mod settings;
mod style;

/// The Dutch texts.
const DUTCH: &str = include_str!("../i18n/nl.toml");

fn main() -> eframe::Result {
    if std::env::args_os().len() > 1 {
        attach_console();
    }
    let args = cli::parse();
    if args.is_command_line() {
        std::process::exit(cli::run(&args));
    }
    if let Some(option) = args.stray_print_options() {
        eprintln!("{option} needs a text to print (see --help)");
        std::process::exit(2);
    }

    crate::i18n::install_dutch(DUTCH);
    if args.resident {
        run_resident();
        return Ok(());
    }
    let theme = if args.dark {
        Some(egui::Theme::Dark)
    } else if args.light {
        Some(egui::Theme::Light)
    } else {
        None
    };

    // One window: a second start brings the open one forward.
    let window = if args.screenshot.is_some() {
        None
    } else {
        match resident::claim(resident::WINDOW_PORT) {
            Some(listener) => Some(listener),
            None if resident::send(resident::WINDOW_PORT, "show") => return Ok(()),
            None => None,
        }
    };
    app::run(args.window_text, args.screenshot, theme, window)
}

/// Output to the console the exe was started from (cmd, PowerShell): a window program has
/// none of its own. Where the output already goes somewhere (a pipe, a file), it stays there.
/// The one `unsafe` of the workspace's own code: a Windows call without a safe wrapper.
#[allow(unsafe_code)]
fn attach_console() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        // SAFETY: no arguments to get wrong; fails harmlessly without a parent console.
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

/// The background process of resident mode; why it stopped goes to `resident.log` next to
/// the settings.
fn run_resident() {
    #[cfg(windows)]
    {
        let settings = settings::Settings::load();
        crate::i18n::set_language(if settings.language.is_empty() {
            settings::system_language()
        } else {
            crate::i18n::Language::from_code(&settings.language)
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
        let missing =
            crate::i18n::check::missing(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), DUTCH);
        assert!(
            missing.is_empty(),
            "not in i18n/nl.toml:\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn dutch_keeps_the_placeholders() {
        let wrong = crate::i18n::check::placeholder_mismatches(DUTCH);
        assert!(wrong.is_empty(), "other placeholders: {wrong:?}");
    }
}
