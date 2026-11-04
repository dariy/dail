//! Plain-text bodies read as documents (MISC-28).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::document::{Block, FoldKind, LinkNote, Span};
use app_io::plain::{inline, plain_document};

fn text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

#[test]
fn wrapped_lines_join_and_short_lines_keep_their_break() {
    let body = "Hi Alex,\n\nI pushed the revised onboarding flow to Figma. The main change is that\naccount setup now takes three steps instead of five.\n\n\n\nThanks,\nPriya\n";
    let doc = plain_document(body);
    let paragraphs: Vec<String> = doc
        .blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph { spans } => text(spans),
            other => panic!("not a paragraph: {other:?}"),
        })
        .collect();
    assert_eq!(
        paragraphs,
        [
            "Hi Alex,",
            "I pushed the revised onboarding flow to Figma. The main change is that account setup now takes three steps instead of five.",
            "Thanks,\nPriya",
        ],
        "three blank lines are one gap"
    );
}

#[test]
fn headings_by_markdown_underline_capitals_and_guess() {
    let body = "# Release notes\n\nOverview\n========\n\nIMPORTANT CHANGES\n\nNew in this version\n\nThe sync engine was rewritten from scratch to be faster and to use less data\nwhen the network is metered.\n\nThanks\n";
    let doc = plain_document(body);
    let headings: Vec<(u8, String)> = doc
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Heading { level, spans } => Some((*level, text(spans))),
            _ => None,
        })
        .collect();
    assert_eq!(
        headings,
        [
            (1, "Release notes".to_owned()),
            (1, "Overview".to_owned()),
            (3, "IMPORTANT CHANGES".to_owned()),
            (3, "New in this version".to_owned()),
        ],
        "a closing word alone is not a heading"
    );
}

#[test]
fn lists_quotes_code_and_rules() {
    let body = "Steps:\n\n- one\n- two\n  continued\n\n1. first\n2. second\n\n> quoted\n> text\n\n```\nlet x = 1;\n```\n\n---\n";
    let doc = plain_document(body);
    let kinds: Vec<&str> = doc
        .blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph { .. } => "p",
            Block::List { ordered: false, .. } => "ul",
            Block::List { ordered: true, .. } => "ol",
            Block::Quote { .. } => "quote",
            Block::Preformatted { .. } => "pre",
            Block::Rule => "hr",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["p", "ul", "ol", "quote", "pre", "hr"]);
    let Block::List { items, .. } = &doc.blocks[1] else {
        unreachable!()
    };
    let Block::Paragraph { spans } = &items[1][0] else {
        unreachable!()
    };
    assert_eq!(text(spans), "two continued");
    let Block::Preformatted { text } = &doc.blocks[4] else {
        unreachable!()
    };
    assert_eq!(text, "let x = 1;");
}

#[test]
fn inline_markdown_and_what_stays_text() {
    let spans = inline("A **bold** and *soft* `code` ~~gone~~ snake_case_name 2 * 3 (1) [1]");
    let styled: Vec<(&str, bool, bool, bool, bool)> = spans
        .iter()
        .map(|s| (s.text.as_str(), s.bold, s.italic, s.code, s.strike))
        .collect();
    assert_eq!(
        styled,
        [
            ("A ", false, false, false, false),
            ("bold", true, false, false, false),
            (" and ", false, false, false, false),
            ("soft", false, true, false, false),
            (" ", false, false, false, false),
            ("code", false, false, true, false),
            (" ", false, false, false, false),
            ("gone", false, false, false, true),
            (" snake_case_name 2 * 3 (1) [1]", false, false, false, false),
        ]
    );
}

/// Links get numbers as separate markers; a bare URL is the marker alone; a sender's `(1)` stays text.
#[test]
fn links_are_numbered_markers() {
    let body = "See [the activity](https://example.org/a) (1) and https://example.org/b.\nAgain: [here](https://example.org/a), and <mailto:x@example.org>, but not javascript:alert(1) or [x](javascript:y).\n";
    let doc = plain_document(body);
    let Block::Paragraph { spans } = &doc.blocks[0] else {
        panic!()
    };
    let marked: Vec<(&str, Option<&str>, Option<u32>)> = spans
        .iter()
        .filter(|s| s.link.is_some())
        .map(|s| (s.text.as_str(), s.link.as_deref(), s.note))
        .collect();
    assert_eq!(
        marked,
        [
            ("the activity", Some("https://example.org/a"), Some(1)),
            ("", Some("https://example.org/b"), Some(2)),
            ("here", Some("https://example.org/a"), Some(1)),
            ("", Some("mailto:x@example.org"), Some(3)),
        ]
    );
    assert!(
        text(spans).contains(" (1) and "),
        "the sender's (1) is text"
    );
    assert!(text(spans).contains("javascript:alert(1)"));
    assert_eq!(
        doc.notes(),
        [
            LinkNote {
                number: 1,
                url: "https://example.org/a".into(),
                title: "the activity".into()
            },
            LinkNote {
                number: 2,
                url: "https://example.org/b".into(),
                title: String::new()
            },
            LinkNote {
                number: 3,
                url: "mailto:x@example.org".into(),
                title: String::new()
            },
        ]
    );
}

#[test]
fn signatures_and_quotes_fold() {
    let body = "Sounds good.\n\n-- \nAlex Morgan\nNorthwind\n\nOn Monday, Priya wrote:\n> Shall we meet?\n";
    let doc = plain_document(body);
    let folds: Vec<FoldKind> = doc
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Fold { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect();
    assert_eq!(folds, [FoldKind::Signature, FoldKind::Quote]);
}

/// HTML documents number their links the same way, and an anchor whose text is its target is the marker alone.
#[test]
fn html_links_are_numbered_too() {
    let raw = b"Content-Type: text/html\r\n\r\n<p><a href=\"https://example.org/x\">Check activity</a> or <a href=\"https://example.org/y\">https://example.org/y</a></p>";
    let (_, doc) = app_io::mime::parse_message_with_document(raw);
    let notes = doc.unwrap().notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].title, "Check activity");
    assert_eq!(notes[1].title, "");
}
