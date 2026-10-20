//! Untrusted text for the terminal. Email content (subjects, names, addresses, bodies, attachment
//! names) and an agent's draft text are attacker-controlled: drawn as they are, `ESC` sequences
//! could clear the screen, set the window title or the clipboard (OSC 52), and bidi overrides
//! could make a link read as another one. Every widget draws such text through [`clean`] (one
//! line) or [`clean_lines`] (a body).

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

/// The display width of `text` in terminal columns.
pub fn width(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

fn char_width(c: char) -> usize {
    unicode_width::UnicodeWidthChar::width(c).unwrap_or(0)
}

/// `text` (one clean line) wrapped at word boundaries into lines of at most `columns` columns;
/// a word longer than a line is split. An empty line stays one empty line.
pub fn wrap(text: &str, columns: usize) -> Vec<String> {
    let columns = columns.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut used = 0;
    for word in text.split_inclusive(' ') {
        let w = width(word);
        let trimmed = width(word.trim_end_matches(' '));
        if used > 0 && used + trimmed > columns {
            lines.push(std::mem::take(&mut line).trim_end().to_owned());
            used = 0;
        }
        // Trailing spaces may hang over the edge; they are trimmed when the line ends.
        if used + trimmed <= columns {
            line.push_str(word);
            used += w;
            continue;
        }
        // A word wider than the line: split it.
        for c in word.chars() {
            let cw = char_width(c);
            if used + cw > columns && used > 0 {
                lines.push(std::mem::take(&mut line).trim_end().to_owned());
                used = 0;
            }
            line.push(c);
            used += cw;
        }
    }
    lines.push(line.trim_end().to_owned());
    lines
}

/// `text` cut or padded to exactly `columns` columns (cut text ends in `…`).
pub fn pad(text: &str, columns: usize) -> String {
    let total = width(text);
    let mut out = String::new();
    let mut used = 0;
    if total <= columns {
        out.push_str(text);
        used = total;
    } else {
        for c in text.chars() {
            let cw = char_width(c);
            if used + cw + 1 > columns {
                break;
            }
            out.push(c);
            used += cw;
        }
        if columns > 0 {
            out.push('…');
            used += 1;
        }
    }
    out.extend(std::iter::repeat_n(' ', columns.saturating_sub(used)));
    out
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
    fn wrapping_and_padding_count_columns() {
        assert_eq!(wrap("the quick brown fox", 9), ["the quick", "brown fox"]);
        assert_eq!(
            wrap("https://example.org/long", 10),
            ["https://ex", "ample.org/", "long"]
        );
        assert_eq!(wrap("", 10), [""]);
        assert_eq!(wrap("日本語の文", 4), ["日本", "語の", "文"]);
        assert_eq!(pad("Carol", 7), "Carol  ");
        assert_eq!(pad("Christopher", 7), "Christ…");
        assert_eq!(pad("日本語", 5), "日本…");
    }
}
