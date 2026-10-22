//! Text for the terminal. The sanitizer for untrusted text lives in [`app_front::text`] and is
//! re-exported here: every widget draws email content through [`clean`] (one line) or
//! [`clean_lines`] (a body). Widths, wrapping and padding count terminal columns.

pub use app_front::text::{clean, clean_lines, is_unsafe};

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
