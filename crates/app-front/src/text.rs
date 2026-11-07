//! Untrusted text for every front-end. Email content (subjects, names, addresses, bodies,
//! attachment names) and an agent's draft text are attacker-controlled: drawn as they are, `ESC`
//! sequences could clear a terminal or set its clipboard (OSC 52), and bidi overrides could make
//! a link read as another one. The TUI and the mobile frames pass such text through [`clean`]
//! (one line) or [`clean_lines`] (a body).

/// Width of a tab stop.
const TAB: usize = 8;

/// What a dangerous character becomes.
const REPLACEMENT: char = '\u{FFFD}';

/// Whether `c` must not reach the terminal: C0 and C1 controls (ESC among them), DEL, and the
/// bidi embeddings, overrides and isolates (U+202A–U+202E, U+2066–U+2069) and marks.
pub const fn is_unsafe(c: char) -> bool {
    matches!(c,
        '\u{0}'..='\u{1f}'
        | '\u{7f}'..='\u{9f}'
        | '\u{202a}'..='\u{202e}'
        | '\u{2066}'..='\u{2069}'
        | '\u{200e}' | '\u{200f}' | '\u{061c}'
    )
}

/// One line of untrusted text: tabs expanded, line breaks as spaces, every other unsafe
/// character replaced by U+FFFD.
pub fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut column = 0;
    for c in text.chars() {
        match c {
            '\t' => {
                let pad = TAB - column % TAB;
                out.extend(std::iter::repeat_n(' ', pad));
                column += pad;
            }
            '\n' | '\r' => {
                out.push(' ');
                column += 1;
            }
            c if is_unsafe(c) => {
                out.push(REPLACEMENT);
                column += 1;
            }
            c => {
                out.push(c);
                column += 1;
            }
        }
    }
    out
}

/// A multi-line untrusted text, one cleaned string per line (`\r\n`, `\n` and a lone `\r` end a
/// line).
pub fn clean_lines(text: &str) -> Vec<String> {
    text.split('\n')
        .flat_map(|line| line.strip_suffix('\r').unwrap_or(line).split('\r'))
        .map(clean)
        .collect()
}

/// Licence text hard-wrapped at a fixed width, rewrapped for the screen. The lines of each
/// paragraph are joined so that the screen's own soft wrap takes over. Blank lines, centred
/// or upper-case headings, rules, `//` comments, list items (`1.`, `(a)`, `-`, `*`) and code fences
/// keep their own lines.
pub fn reflow(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut open = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if open {
                out.push('\n');
            }
            out.push('\n');
            open = false;
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        // A heading or rule (no lower-case letter) and a code comment stand alone too.
        let alone = trimmed.starts_with("```")
            || trimmed.starts_with("//")
            || indent >= 8
            || !trimmed.chars().any(char::is_lowercase);
        if open && !alone && !starts_item(trimmed) {
            out.push(' ');
        } else if open {
            out.push('\n');
        }
        out.push_str(trimmed);
        open = !alone;
        if alone {
            out.push('\n');
        }
    }
    out.trim().to_owned()
}

/// Whether a line starts a list item: `1.`, `1)`, `(a)`, `(iv)`, `-` or `*`.
fn starts_item(line: &str) -> bool {
    if line.starts_with("- ") || line.starts_with("* ") {
        return true;
    }
    if let Some(rest) = line.strip_prefix('(') {
        let label: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        return (1..=4).contains(&label.len()) && rest[label.len()..].starts_with(')');
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && (line[digits..].starts_with(". ") || line[digits..].starts_with(") "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_and_bidi_characters_never_survive() {
        let nasty = "a\u{1b}]52;c;ZXZpbA==\u{7}b\u{1b}[2Jc\u{9b}d\u{7f}e\u{202e}f\u{2067}g";
        let cleaned = clean(nasty);
        assert!(!cleaned.chars().any(is_unsafe), "{cleaned:?}");
        assert!(cleaned.starts_with('a') && cleaned.ends_with('g'));
        assert_eq!(clean("a\tb"), "a       b");
        assert_eq!(clean("12345678\tx"), "12345678        x");
        assert_eq!(clean("one\r\ntwo"), "one  two");
        assert_eq!(
            clean_lines("one\r\ntwo\rthree\n"),
            ["one", "two", "three", ""]
        );
        assert_eq!(clean("Grüße, 日本"), "Grüße, 日本");
    }

    #[test]
    fn reflow_joins_hard_wrapped_lines_and_keeps_structure() {
        let text = "                Apache License\n\n   1. Definitions.\n\n      \"License\" shall mean\n      the terms.\n\n   (a) You must give\n       a copy; and\n\n   (b) You must\nkeep notices.\n";
        assert_eq!(
            reflow(text),
            "Apache License\n\n1. Definitions.\n\n\"License\" shall mean the terms.\n\n(a) You must give a copy; and\n\n(b) You must keep notices."
        );
        assert_eq!(reflow("a\nb\n- one\n- two"), "a b\n- one\n- two");
        assert_eq!(
            reflow("---\nPREAMBLE\nThe goals\nof it"),
            "---\nPREAMBLE\nThe goals of it"
        );
        assert_eq!(
            reflow("```\nCopyright x\n Use of this\n found in\n```"),
            "```\nCopyright x Use of this found in\n```"
        );
    }
}
