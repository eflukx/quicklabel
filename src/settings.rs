//! What quicklabel remembers: the printers and the language.

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
}

impl Settings {
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
pub fn system_language() -> chopshop_i18n::Language {
    let dutch = sys_locale::get_locale()
        .is_some_and(|locale| locale.to_ascii_lowercase().starts_with("nl"));
    if dutch {
        chopshop_i18n::Language::Dutch
    } else {
        chopshop_i18n::Language::English
    }
}
