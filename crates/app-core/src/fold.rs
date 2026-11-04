//! Folding of plain-text bodies: the quoted earlier messages at the end of a reply and the
//! sender's signature, which the reader hides until asked (HTML bodies carry their folds in the
//! document, `app_core::document::Block::Fold`).
//!
//! Quoted text is the trailing run of `>` lines (with the attribution line before it, such as
//! "On Monday, Alice wrote:"), or everything from an Outlook "-----Original Message-----" or
//! "From:" header block on. A signature starts at a `-- ` line and is at most 15 lines long. Only
//! the end of a message is folded; replies written between quoted lines stay as they are.

/// A signature longer than this many lines is not folded (it is probably not one).
const MAX_SIGNATURE_LINES: usize = 15;

/// A body split into what is shown and what is folded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Folded<'a> {
    /// The message's own text.
    pub main: &'a str,
    /// The signature, from its `-- ` line on.
    pub signature: Option<&'a str>,
    /// The quoted earlier messages, from the attribution line on.
    pub quote: Option<&'a str>,
}

/// Splits `text` into its own text, signature and quoted part.
pub fn split(text: &str) -> Folded<'_> {
    let lines = line_starts(text);
    let quote_at = quote_start(text, &lines).filter(|&at| !text[..at].trim().is_empty());
    let (head, quote) = match quote_at {
        Some(at) => (&text[..at], Some(text[at..].trim_end())),
        None => (text, None),
    };
    let head_lines = line_starts(head);
    let signature_at = head_lines
        .iter()
        .rposition(|&(start, end)| matches!(head[start..end].trim_end_matches('\r'), "-- " | "--"))
        .filter(|&i| i > 0 && head_lines.len() - i <= MAX_SIGNATURE_LINES + 1)
        .and_then(|i| head_lines.get(i))
        .map(|&(start, _)| start)
        .filter(|&at| !head[..at].trim().is_empty());
    let (main, signature) = match signature_at {
        Some(at) => (&head[..at], Some(head[at..].trim_end())),
        None => (head, None),
    };
    Folded {
        main: main.trim_end(),
        signature: signature.filter(|s| !s.is_empty()),
        quote: quote.filter(|q| !q.is_empty()),
    }
}

/// `(start, end)` byte offsets of each line, without the `\n`.
fn line_starts(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if c == '\n' {
            out.push((start, i));
            start = i + 1;
        }
    }
    if start < text.len() {
        out.push((start, text.len()));
    }
    out
}

fn is_quoted(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

/// Where the quoted part starts, as a byte offset.
fn quote_start(text: &str, lines: &[(usize, usize)]) -> Option<usize> {
    let line = |i: usize| {
        lines
            .get(i)
            .map_or("", |&(s, e)| text[s..e].trim_end_matches('\r'))
    };
    // An Outlook header block: "-----Original Message-----", or a rule of underscores followed by
    // "From:".
    let outlook = (0..lines.len()).find(|&i| {
        let l = line(i).trim();
        (l.starts_with("-----") && l.to_ascii_lowercase().contains("original message"))
            || (l.len() >= 20
                && l.chars().all(|c| c == '_')
                && line(i + 1).trim_start().starts_with("From:"))
    });
    // The trailing run of quoted (or blank) lines.
    let last = (0..lines.len())
        .rev()
        .find(|&i| !line(i).trim().is_empty())?;
    let mut first = last + 1;
    while first > 0 && (is_quoted(line(first - 1)) || line(first - 1).trim().is_empty()) {
        first -= 1;
    }
    let trailing = (first <= last && (first..=last).any(|i| is_quoted(line(i)))).then(|| {
        // Skip the blank lines the run started with.
        let mut start = first;
        while line(start).trim().is_empty() {
            start += 1;
        }
        // The attribution line ("On …, Alice wrote:"), which may wrap onto a second line.
        let mut before = start;
        if before > 0 && line(before - 1).trim_end().ends_with(':') {
            before -= 1;
            if before > 0 {
                let previous = line(before - 1).trim_start();
                let opens = |l: &str| {
                    ["On ", "Am ", "Le ", "El ", "Il ", "Op ", "Em "]
                        .iter()
                        .any(|o| l.starts_with(o))
                };
                if !opens(line(before).trim_start()) && opens(previous) {
                    before -= 1;
                }
            }
        }
        before
    });
    let at = match (outlook, trailing) {
        (Some(a), Some(b)) => a.min(b),
        (a, b) => a.or(b)?,
    };
    lines.get(at).map(|&(start, _)| start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_quotes_fold_with_their_attribution() {
        let text = "Sounds good.\n\nOn Mon, 13 Nov 2023 at 10:00, Alice\n<alice@x> wrote:\n> Lunch?\n>\n> A\n";
        let folded = split(text);
        assert_eq!(folded.main, "Sounds good.");
        assert_eq!(
            folded.quote,
            Some("On Mon, 13 Nov 2023 at 10:00, Alice\n<alice@x> wrote:\n> Lunch?\n>\n> A")
        );
        assert_eq!(folded.signature, None);
    }

    #[test]
    fn signatures_and_outlook_headers() {
        let text = "Yes.\n\n-- \nBob\nCTO\n\n-----Original Message-----\nFrom: Alice\nLunch?";
        let folded = split(text);
        assert_eq!(folded.main, "Yes.");
        assert_eq!(folded.signature, Some("-- \nBob\nCTO"));
        assert_eq!(
            folded.quote,
            Some("-----Original Message-----\nFrom: Alice\nLunch?")
        );

        let rule = format!("Ok\n{}\nFrom: Alice\nSent: Monday\n\nHi", "_".repeat(32));
        assert_eq!(split(&rule).main, "Ok");
    }

    #[test]
    fn interleaved_replies_and_whole_quotes_stay() {
        let interleaved = "> Lunch?\nYes.\n> Where?\nThe usual.";
        assert_eq!(split(interleaved).main, interleaved);
        assert_eq!(split(interleaved).quote, None);
        let only = "> forwarded\n> text";
        assert_eq!(split(only).main, only, "nothing of its own: nothing folded");
        let long = format!("Hi\n--\n{}", "line\n".repeat(20));
        assert_eq!(split(&long).signature, None);
        assert_eq!(
            split(""),
            Folded {
                main: "",
                signature: None,
                quote: None
            }
        );
        assert_eq!(split("-- \nonly a signature").signature, None);
    }
}
