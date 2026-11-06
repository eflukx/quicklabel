//! Ctrl+B and Ctrl+I: bold or italic on and off around the selection.
//!
//! How to use: `toggle(text, selection, Style::Bold)` gives the new text and the new selection
//! (character indices). Selected `x` becomes `**x**`, and again `x`; a selection of
//! `**x**` or the `x` inside `**x**` both lose the stars. Bold and italic are separate: `***x***`
//! with italic off is `**x**`. Nothing selected: the stars go in with the cursor between them.
//! A selection over several lines works per line (the markdown is per line); empty lines stay.
//!
//! Working principle: around the selected text, the stars right inside the selection and right
//! outside it count together as one run on each side; the shorter side's run is the style
//! (odd: italic, two or more: bold). The run is changed by 1 (italic) or 2 (bold) and the stars
//! are written outside the selected text, which stays selected.

use std::ops::Range;

/// The style to switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// `**`.
    Bold,
    /// `*`.
    Italic,
}

impl Style {
    fn stars(self) -> usize {
        match self {
            Self::Bold => 2,
            Self::Italic => 1,
        }
    }

    fn is_in(self, run: usize) -> bool {
        match self {
            Self::Bold => run >= 2,
            Self::Italic => run % 2 == 1,
        }
    }
}

/// `text` with `style` switched for the `selection` (char indices), and the new selection.
#[must_use]
pub fn toggle(text: &str, selection: Range<usize>, style: Style) -> (String, Range<usize>) {
    let chars: Vec<char> = text.chars().collect();
    let (start, end) = (
        selection.start.min(chars.len()),
        selection.end.min(chars.len()),
    );
    let selected: String = chars[start..end].iter().collect();

    if selected.contains('\n') {
        return toggle_lines(&chars, start, end, style);
    }

    let before: String = chars[..start].iter().collect();
    let after: String = chars[end..].iter().collect();
    let (word, runs) = split(&before, &selected, &after);
    let run = runs.0.min(runs.1);
    let new_run = if style.is_in(run) {
        run - style.stars()
    } else {
        run + style.stars()
    };

    // The stars of the run go; the new run is written right around the word.
    let before_kept = &before[..before.len() - runs.2];
    let after_kept = &after[runs.3..];
    let stars = "*".repeat(new_run);
    let left_extra = "*".repeat(runs.0 - run);
    let right_extra = "*".repeat(runs.1 - run);
    let out = format!("{before_kept}{left_extra}{stars}{word}{stars}{right_extra}{after_kept}");
    let word_start = before_kept.chars().count() + left_extra.len() + new_run;
    (out, word_start..word_start + word.chars().count())
}

/// The word inside the stars, and the runs: (left, right, stars taken from `before`, stars
/// taken from `after`).
fn split(before: &str, selected: &str, after: &str) -> (String, (usize, usize, usize, usize)) {
    let inside_left = selected.chars().take_while(|&c| c == '*').count();
    let word_and_right = &selected[inside_left..];
    let inside_right = word_and_right
        .chars()
        .rev()
        .take_while(|&c| c == '*')
        .count();
    // A selection of only stars: all on the left.
    let inside_right = inside_right.min(word_and_right.len());
    let word = &word_and_right[..word_and_right.len() - inside_right];

    let outside_left = before.chars().rev().take_while(|&c| c == '*').count();
    let outside_right = after.chars().take_while(|&c| c == '*').count();
    (
        word.to_owned(),
        (
            outside_left + inside_left,
            outside_right + inside_right,
            outside_left,
            outside_right,
        ),
    )
}

/// Several lines: every non-empty line of the selection gets the style, or all lose it when
/// all have it. The selection then spans the changed lines.
fn toggle_lines(chars: &[char], start: usize, end: usize, style: Style) -> (String, Range<usize>) {
    // Whole lines from the selection's first to its last.
    let first = chars[..start]
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |at| at + 1);
    let last = chars[end..]
        .iter()
        .position(|&c| c == '\n')
        .map_or(chars.len(), |at| end + at);
    let block: String = chars[first..last].iter().collect();

    let has = |line: &str| {
        let (_, runs) = split("", line.trim(), "");
        style.is_in(runs.0.min(runs.1))
    };
    let lines: Vec<&str> = block.split('\n').collect();
    let all = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .all(|line| has(line));

    let changed: Vec<String> = lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() || has(line) != all {
                return (*line).to_owned();
            }
            let length = line.chars().count();
            toggle(line, 0..length, style).0
        })
        .collect();
    let block = changed.join("\n");
    let head: String = chars[..first].iter().collect();
    let tail: String = chars[last..].iter().collect();
    let count = block.chars().count();
    (format!("{head}{block}{tail}"), first..first + count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, selection: Range<usize>, style: Style) -> (String, String) {
        let (out, range) = toggle(text, selection, style);
        let selected = out.chars().skip(range.start).take(range.len()).collect();
        (out, selected)
    }

    #[test]
    fn bold_on_and_off() {
        let (text, selected) = run("a big box", 2..5, Style::Bold);
        assert_eq!((text.as_str(), selected.as_str()), ("a **big** box", "big"));
        // The same selection (the word) again: off.
        let (text, _) = run(&text, 4..7, Style::Bold);
        assert_eq!(text, "a big box");
        // The stars selected too: off as well.
        assert_eq!(run("a **big** box", 2..9, Style::Bold).0, "a big box");
    }

    #[test]
    fn bold_and_italic_are_separate() {
        assert_eq!(run("**x**", 2..3, Style::Italic).0, "***x***");
        assert_eq!(run("***x***", 3..4, Style::Italic).0, "**x**");
        assert_eq!(run("***x***", 3..4, Style::Bold).0, "*x*");
        assert_eq!(run("*x*", 1..2, Style::Italic).0, "x");
    }

    #[test]
    fn nothing_selected_puts_the_cursor_between() {
        let (text, range) = toggle("ab", 1..1, Style::Bold);
        assert_eq!(text, "a****b");
        assert_eq!(range, 3..3);
    }

    #[test]
    fn several_lines_each_get_it() {
        let text = "one\n\ntwo";
        let (out, range) = toggle(text, 0..8, Style::Bold);
        assert_eq!(out, "**one**\n\n**two**");
        assert_eq!(range, 0..out.chars().count());
        assert_eq!(toggle(&out, range, Style::Bold).0, text);
        // Mixed: all get it.
        assert_eq!(
            toggle("**one**\ntwo", 0..11, Style::Bold).0,
            "**one**\n**two**"
        );
    }

    #[test]
    fn other_characters_count_as_characters() {
        let (text, selected) = run("é 😀 ö", 2..3, Style::Italic);
        assert_eq!((text.as_str(), selected.as_str()), ("é *😀* ö", "😀"));
    }
}
