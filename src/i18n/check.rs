//! Checks for an app's test: every text in its code has a Dutch one, with the same placeholders.

use std::path::Path;

/// The texts in `t("…")` and `tf("…"` calls of one source file.
#[must_use]
pub fn texts_in(source: &str) -> Vec<String> {
    let mut texts = Vec::new();
    for call in ["t(", "tf(", "n("] {
        let mut rest = source;
        while let Some(start) = rest.find(call) {
            // A call, not the end of another name (`print(`, `start(`).
            let before = rest[..start].chars().last();
            rest = &rest[start + call.len()..];
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            // The text may start on the next line (rustfmt).
            let Some(after) = rest.trim_start().strip_prefix('"') else {
                continue;
            };
            rest = after;
            let mut text = String::new();
            let mut chars = rest.chars();
            while let Some(c) = chars.next() {
                match c {
                    '"' => break,
                    '\\' => match chars.next() {
                        Some('n') => text.push('\n'),
                        Some('\n') => {
                            // A line continuation: the next line's indent goes.
                            let tail: String = chars.clone().collect();
                            let skipped = tail.len() - tail.trim_start().len();
                            for _ in 0..skipped {
                                chars.next();
                            }
                        }
                        Some(other) => text.push(other),
                        None => {}
                    },
                    other => text.push(other),
                }
            }
            texts.push(text);
        }
    }
    texts
}

fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source dir") {
        let path = entry.expect("entry").path();
        // This module's own examples and test texts aren't the app's.
        if path.is_dir() && path.file_name().is_some_and(|name| name == "i18n") {
            continue;
        }
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The `t`, `tf` and `n` texts in the `.rs` files under `dir` that `catalog` lacks, as
/// `file: "text"`.
#[must_use]
pub fn missing(dir: &Path, catalog: &str) -> Vec<String> {
    let entries = super::parse(catalog).unwrap_or_default();
    let mut files = Vec::new();
    sources(dir, &mut files);

    let mut missing = Vec::new();
    for file in files {
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };
        for text in texts_in(&source) {
            if !entries.contains_key(&text) {
                missing.push(format!("{}: {text:?}", file.display()));
            }
        }
    }
    missing
}

/// Catalog entries whose Dutch has other `{placeholders}` than the English.
#[must_use]
pub fn placeholder_mismatches(catalog: &str) -> Vec<String> {
    let names = |text: &str| {
        let mut names: Vec<String> = text
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(name, _)| name.to_owned()))
            .collect();
        names.sort();
        names
    };
    super::parse(catalog)
        .unwrap_or_default()
        .iter()
        .filter(|(english, dutch)| names(english) != names(dutch))
        .map(|(english, _)| english.clone())
        .collect()
}
