//! `quicklabel.com`: Quick label from cmd and PowerShell, waiting for it.
//!
//! How to use: installed next to `quicklabel.exe` as `quicklabel.com`. Typing `quicklabel …`
//! in cmd or PowerShell runs this one (`.com` comes before `.exe`), the Start menu and the
//! hotkey the exe.
//!
//! Working principle: `quicklabel.exe` is a window program, so cmd doesn't wait for it: its
//! output came after the prompt, its exit code was lost. This console program starts the exe
//! next to it with the same arguments and the same console, waits, and exits with its code.
//! Opening the window (no options, or only `--text`, `--dark`, `--light`) it doesn't wait for.

use std::process::{Command, ExitCode};

/// The window's options (the one with a value: `--text`).
fn opens_the_window(args: &[String]) -> bool {
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--dark" | "--light" => {}
            "--text" => {
                rest.next();
            }
            _ => return false,
        }
    }
    true
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let exe = std::env::current_exe()
        .map(|path| path.with_file_name("quicklabel.exe"))
        .unwrap_or_else(|_| "quicklabel.exe".into());
    let mut child = match Command::new(&exe).args(&args).spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!("{}: {error}", exe.display());
            return ExitCode::from(1);
        }
    };
    if opens_the_window(&args) {
        return ExitCode::SUCCESS;
    }
    match child.wait() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1).clamp(0, 255) as u8),
        Err(error) => {
            eprintln!("{}: {error}", exe.display());
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::opens_the_window;

    #[test]
    fn only_window_options_open_the_window() {
        let args = |line: &[&str]| line.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        assert!(opens_the_window(&args(&[])));
        assert!(opens_the_window(&args(&["--dark", "--text", "-l"])));
        assert!(!opens_the_window(&args(&["-l"])));
        assert!(!opens_the_window(&args(&["Rack 4"])));
        assert!(!opens_the_window(&args(&["--help"])));
    }
}
