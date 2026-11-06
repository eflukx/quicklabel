//! Numbered labels: `{n}` in the text becomes the label's number in a print run.
//!
//! How to use: `fill("Box {n}", 0)` is `Box 1`, `fill("Box {n}", 2)` is `Box 3`. `{n+12}` adds 12:
//! 13, 14, …; an offset with leading zeros keeps its width: `{n+007}` gives 008, 009, ….
//! Spaces inside the braces are allowed (`{n + 12}`). Anything else in braces stays as typed.

/// `text` with every `{n}` / `{n+offset}` replaced for the label `index` (0 for the first).
#[must_use]
pub fn fill(text: &str, index: u16) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        match after
            .find('}')
            .and_then(|close| number(&after[1..close], index).map(|number| (number, close)))
        {
            Some((number, close)) => {
                out.push_str(&number);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The number for `inside` the braces (`n`, `n+12`, `n + 007`), or `None` when it isn't one.
fn number(inside: &str, index: u16) -> Option<String> {
    let inside: String = inside.chars().filter(|c| !c.is_whitespace()).collect();
    let start = inside.strip_prefix('n')?;
    if start.is_empty() {
        return Some((u64::from(index) + 1).to_string());
    }
    let digits = start.strip_prefix('+')?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let offset: u64 = digits.parse().ok()?;
    let width = if digits.starts_with('0') {
        digits.len()
    } else {
        0
    };
    Some(format!("{:0width$}", offset + u64::from(index) + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_n_counts_from_one() {
        assert_eq!(fill("Box {n}", 0), "Box 1");
        assert_eq!(fill("Box {n}", 9), "Box 10");
        assert_eq!(fill("{n}/{n}", 1), "2/2");
    }

    #[test]
    fn an_offset_is_added() {
        assert_eq!(fill("Rack {n+12}", 0), "Rack 13");
        assert_eq!(fill("Rack {n + 12}", 2), "Rack 15");
        assert_eq!(fill("{n+007}", 0), "008");
        assert_eq!(fill("{n+007}", 2), "010");
        assert_eq!(fill("{n+0}", 0), "1");
        assert_eq!(fill("{n+00}", 0), "01");
        assert_eq!(fill("{n+00}", 98), "99");
        assert_eq!(fill("{n+00}", 99), "100");
        assert_eq!(fill("{n+000}", 0), "001");
    }

    #[test]
    fn other_braces_stay() {
        assert_eq!(fill("{x} {n-1} {n+} {", 0), "{x} {n-1} {n+} {");
        assert_eq!(fill("**{n}**", 0), "**1**");
    }
}
