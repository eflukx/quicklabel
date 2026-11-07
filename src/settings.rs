//! What quicklabel remembers: the printers, the language, light or dark, and per tape how a
//! label is laid out.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Printer addresses (host name or IP), in order of preference.
    pub printers: Vec<String>,
    /// `en` or `nl`; empty: from the system (Dutch Windows: Dutch).
    pub language: String,
    /// `light` or `dark`; empty: as Windows is set.
    pub theme: String,
    /// The text of the last run: shown selected at the next start.
    pub last_text: String,
    /// Close the window after a label printed; else it stays open for the next one.
    pub close_after_print: bool,
    /// Resident mode: a background process holds the hotkey (Windows).
    pub resident: bool,
    /// The hotkey, as Windows writes it (`CTRL+ALT+L`); empty: Ctrl+Alt+L.
    pub hotkey: String,
    /// The typeface of the labels (`barlow`, `roboto`, `serif`); empty: Barlow.
    pub typeface: String,
    /// Per printer address: how it prints.
    pub printer_options: BTreeMap<String, PrinterOptions>,
    /// How labels are laid out, per tape (the media id: `62`, `62x29`, `29`, …).
    pub layouts: BTreeMap<String, Layout>,
}

/// How a label is laid out on one tape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// The text along the label's short side instead of its long side.
    pub sideways: bool,
    /// Continuous tape: the label at most this many times the tape's width long; 0: no limit.
    pub length: u8,
    /// Where the cutter cuts.
    pub cut: Cut,
    /// Every line as large as fills the width, each its own size.
    pub fill_lines: bool,
}

/// How one printer prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrinterOptions {
    /// 600 dpi along the tape, when the model has it: sharper, a little slower.
    pub high_resolution: bool,
}

impl Default for PrinterOptions {
    fn default() -> Self {
        Self {
            high_resolution: true,
        }
    }
}

/// Where the cutter cuts: after each label, once after the batch, or not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cut {
    /// After every label.
    #[default]
    Each,
    /// Once after the last label.
    Batch,
    /// Not at all.
    Off,
}

impl Cut {
    /// The printer job's cut.
    #[must_use]
    pub fn job(self) -> ql_raster::job::Cut {
        match self {
            Self::Each => ql_raster::job::Cut::Each,
            Self::Batch => ql_raster::job::Cut::End,
            Self::Off => ql_raster::job::Cut::Off,
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            sideways: false,
            length: 2,
            cut: Cut::Each,
            fill_lines: false,
        }
    }
}

impl Settings {
    /// How the printer at `host` prints.
    #[must_use]
    pub fn printer(&self, host: &str) -> PrinterOptions {
        self.printer_options.get(host).copied().unwrap_or_default()
    }

    /// The labels' typeface.
    #[must_use]
    pub fn typeface(&self) -> ql_raster::label::Typeface {
        ql_raster::label::Typeface::from_code(&self.typeface)
    }

    /// Where they are kept: `QUICKLABEL_SETTINGS`, else `%APPDATA%\iotta\quicklabel.json`, else
    /// `~/.config/iotta/quicklabel.json`.
    #[must_use]
    pub fn path() -> PathBuf {
        if let Some(path) = std::env::var_os("QUICKLABEL_SETTINGS") {
            return PathBuf::from(path);
        }
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("iotta").join("quicklabel.json")
    }

    /// The saved settings, or the defaults.
    #[must_use]
    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Saves them; the reason when that fails.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        std::fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// The hotkey: the set one, else `CTRL+ALT+L`.
    #[must_use]
    pub fn hotkey(&self) -> &str {
        if self.hotkey.is_empty() {
            "CTRL+ALT+L"
        } else {
            &self.hotkey
        }
    }

    /// The hotkey as people write it: `Ctrl+Alt+L`.
    #[must_use]
    pub fn hotkey_shown(&self) -> String {
        self.hotkey()
            .split('+')
            .map(|part| {
                let mut chars = part.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_string() + &chars.as_str().to_lowercase()
                })
            })
            .collect::<Vec<_>>()
            .join("+")
    }

    /// Where a label goes when there is no printer: a `labels` folder next to the settings.
    #[must_use]
    pub fn labels_dir() -> PathBuf {
        Self::path()
            .parent()
            .map_or_else(|| PathBuf::from("labels"), |dir| dir.join("labels"))
    }
}

/// The language when the settings don't say: Dutch when the system's is (Windows: the user's
/// display language).
#[must_use]
pub fn system_language() -> crate::i18n::Language {
    let dutch = sys_locale::get_locale()
        .is_some_and(|locale| locale.to_ascii_lowercase().starts_with("nl"));
    if dutch {
        crate::i18n::Language::Dutch
    } else {
        crate::i18n::Language::English
    }
}
