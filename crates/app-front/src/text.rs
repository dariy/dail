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
}
