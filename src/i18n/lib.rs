//! GUI texts in more than one language: English (in the code) and Dutch (a catalog).
//!
//! How to use: an app installs its Dutch catalog at start ([`install_dutch`], usually
//! `include_str!` of its `nl.toml`) and picks the language ([`set_language`]). Wrap a text the
//! user reads in [`t`] (`t("Print label")`), or [`tf`] when it has values
//! (`tf("Print again ({n} printed)", &[("n", &count)])`, named `{placeholders}`); mark texts in
//! constant lists with [`n`]. The app's test calls [`check::missing`] and
//! [`check::placeholder_mismatches`], so no text goes without its Dutch one.
//!
//! Working principle: gettext style. The English text is the key; a text the catalog misses
//! shows in English, so technical words may stay English. The language is one global, like a
//! theme.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

pub mod check;

/// A language of the GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// English: the texts in the code.
    English,
    /// Dutch: `assets/i18n/nl.toml`.
    Dutch,
}

impl Language {
    /// Every language, in the order of the switch.
    pub const ALL: [Self; 2] = [Self::English, Self::Dutch];

    /// The code in the settings: `en`, `nl`.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Dutch => "nl",
        }
    }

    /// The language from its code; English for anything else.
    #[must_use]
    pub fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_lowercase().as_str() {
            "nl" | "dutch" | "nederlands" => Self::Dutch,
            _ => Self::English,
        }
    }

    /// The short name on the switch: `EN`, `NL`.
    #[must_use]
    pub fn short(self) -> &'static str {
        match self {
            Self::English => "EN",
            Self::Dutch => "NL",
        }
    }

    /// The language's own name: `English`, `Nederlands`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Dutch => "Nederlands",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Switches the GUI's language.
pub fn set_language(language: Language) {
    CURRENT.store(
        match language {
            Language::English => 0,
            Language::Dutch => 1,
        },
        Ordering::Relaxed,
    );
}

/// The GUI's language.
#[must_use]
pub fn language() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Language::Dutch,
        _ => Language::English,
    }
}

/// The Dutch catalog: English text to Dutch text.
static DUTCH: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Installs the Dutch catalog: a TOML table `"English" = "Nederlands"`. The first call counts.
///
/// # Panics
///
/// When `catalog` isn't a table of strings (it is compiled in, so a test finds that first).
pub fn install_dutch(catalog: &str) {
    DUTCH.get_or_init(|| parse(catalog).expect("the Dutch catalog is a table of strings"));
}

/// The catalog's entries, or why it isn't a table of strings.
pub fn parse(catalog: &str) -> Result<HashMap<String, String>, String> {
    toml::from_str(catalog).map_err(|error| error.to_string())
}

fn dutch(text: &str) -> Option<&'static str> {
    DUTCH.get()?.get(text).map(String::as_str)
}

/// `text` in the GUI's language; the English when the catalog misses it.
#[must_use]
pub fn t(text: &'static str) -> &'static str {
    match language() {
        Language::English => text,
        Language::Dutch => dutch(text).unwrap_or(text),
    }
}

/// [`t`] for a text that isn't `'static` (a name built at runtime).
#[must_use]
pub fn t_dyn(text: &str) -> String {
    match language() {
        Language::English => text.to_owned(),
        Language::Dutch => dutch(text).unwrap_or(text).to_owned(),
    }
}

/// Marks a text in a constant list for the catalog check; [`t`] translates it where shown.
#[must_use]
pub const fn n(text: &'static str) -> &'static str {
    text
}

/// [`t`] with values: each `{name}` replaced by its value.
#[must_use]
pub fn tf(text: &'static str, values: &[(&str, &dyn Display)]) -> String {
    let mut out = t(text).to_owned();
    for (name, value) in values {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_english_and_fills_in_values() {
        install_dutch("\"Print\" = \"Printen\"");
        set_language(Language::Dutch);
        assert_eq!(t("Print"), "Printen");
        assert_eq!(t("Not in the catalog"), "Not in the catalog");
        set_language(Language::English);
        assert_eq!(t("Print"), "Print");
        assert_eq!(tf("{n} devices", &[("n", &3)]), "3 devices");
    }
}
