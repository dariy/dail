//! Marking search words in what the front-ends show: the subject in the thread list and the
//! text in the reader, while a search is open (`view.search`).

use std::ops::Range;

use app_core::Query;
use app_core::search::Condition;

/// The words of a search to mark: its text terms (not the negated ones), in lower case.
pub fn words(search: &str) -> Vec<String> {
    let Ok(query) = Query::parse(search) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for term in query.terms.iter().filter(|t| !t.negated) {
        if let Condition::Text(text) | Condition::Subject(text) = &term.condition {
            let word = text.trim().to_lowercase();
            if !word.is_empty() && !out.contains(&word) {
                out.push(word);
            }
        }
    }
    out
}

/// Where `words` occur in `text`, ignoring case: byte ranges on character boundaries, sorted and
/// merged.
pub fn ranges(text: &str, words: &[String]) -> Vec<Range<usize>> {
    if words.is_empty() {
        return Vec::new();
    }
    // One lower-case character per character of `text` (keeping its byte offset), so that a
    // match maps back onto `text` exactly.
    let chars: Vec<(usize, char)> = text.char_indices().map(|(at, c)| (at, lower(c))).collect();
    let mut found: Vec<Range<usize>> = Vec::new();
    for word in words {
        let needle: Vec<char> = word.chars().map(lower).collect();
        if needle.is_empty() || needle.len() > chars.len() {
            continue;
        }
        for start in 0..=chars.len() - needle.len() {
            if chars[start..start + needle.len()]
                .iter()
                .map(|(_, c)| *c)
                .eq(needle.iter().copied())
            {
                let end = chars
                    .get(start + needle.len())
                    .map_or(text.len(), |(at, _)| *at);
                found.push(chars[start].0..end);
            }
        }
    }
    found.sort_by_key(|r| r.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in found {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// The lower case of `c` if it is one character (else `c`).
fn lower(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// `text` cut into `(piece, marked)` parts by [`ranges`].
pub fn pieces<'a>(text: &'a str, words: &[String]) -> Vec<(&'a str, bool)> {
    let mut out = Vec::new();
    let mut at = 0;
    for range in ranges(text, words) {
        if range.start > at {
            out.push((&text[at..range.start], false));
        }
        out.push((&text[range.clone()], true));
        at = range.end;
    }
    if at < text.len() || out.is_empty() {
        out.push((&text[at..], false));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_come_from_positive_text_terms() {
        assert_eq!(
            words(r#"Budget "q3 numbers" -draft from:bob subject:Plan is:unread"#),
            ["budget", "q3 numbers", "plan"]
        );
        assert!(words("is:unread").is_empty());
    }

    #[test]
    fn matches_ignore_case_and_stay_on_boundaries() {
        let words = vec!["zürich".to_owned(), "q3".to_owned()];
        let text = "Q3 report: ZÜRICH and zürich.";
        let found: Vec<&str> = ranges(text, &words).into_iter().map(|r| &text[r]).collect();
        assert_eq!(found, ["Q3", "ZÜRICH", "zürich"]);
        assert_eq!(
            pieces("a budget!", &["budget".to_owned()]),
            [("a ", false), ("budget", true), ("!", false)]
        );
        assert_eq!(pieces("", &["x".to_owned()]), [("", false)]);
        // Overlapping words are merged.
        let words = vec!["abc".to_owned(), "bcd".to_owned()];
        assert_eq!(ranges("xabcdx", &words), [Range { start: 1, end: 5 }]);
    }
}
