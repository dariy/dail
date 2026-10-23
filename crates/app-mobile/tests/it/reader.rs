//! The reader: collapsed and expanded messages, an HTML body as blocks, plain text with folds,
//! the toolbar, the previous and next conversation, *Links (N)* and the link sheet.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::document::{Block, Document, FoldKind, Span};
use app_core::{Command, ThreadId};
use app_mobile::frame::{DocBlock, DocSpan, Reader};
use app_mobile::{External, Frame, UiEvent};

use crate::support::{Phone, html_body, inbox_of, plain_body};

fn reader(frame: &Frame) -> &Reader {
    frame.reader.as_ref().unwrap()
}

/// The fixture's inbox with `thread` open and its messages answered with `bodies`.
fn open(thread: usize, bodies: Vec<serde_json::Value>) -> (Phone, Vec<ThreadId>) {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.bodies = Some(bodies);
    phone.dispatch(Command::OpenThread {
        id: threads[thread],
    });
    (phone, threads)
}

fn button(reader: &Reader, label: &str) -> u32 {
    reader
        .actions
        .iter()
        .find(|b| b.label == label)
        .unwrap_or_else(|| panic!("no {label} in {:?}", reader.actions))
        .action
}

fn text(spans: &[DocSpan]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

fn span(text: &str) -> Span {
    Span {
        text: text.to_owned(),
        ..Span::default()
    }
}

#[test]
fn an_html_body_is_drawn_as_blocks_with_links_behind_a_sheet() {
    let document = Document {
        blocks: vec![
            Block::Heading {
                level: 1,
                spans: vec![span("Q3")],
            },
            Block::Paragraph {
                spans: vec![
                    span("See "),
                    Span {
                        text: "our bank".to_owned(),
                        link: Some("https://evil.example/login".to_owned()),
                        ..Span::default()
                    },
                    Span {
                        text: " now".to_owned(),
                        bold: true,
                        ..Span::default()
                    },
                ],
            },
            Block::List {
                ordered: true,
                start: 3,
                items: vec![vec![Block::Paragraph {
                    spans: vec![span("one")],
                }]],
            },
            Block::Preformatted {
                text: "a  b\nc".to_owned(),
            },
            Block::Table {
                rows: vec![vec![app_core::document::Cell {
                    header: true,
                    spans: vec![span("Head")],
                }]],
            },
            Block::Rule,
            Block::Quote {
                blocks: vec![Block::Paragraph {
                    spans: vec![span("quoted")],
                }],
            },
            Block::Fold {
                kind: FoldKind::Quote,
                blocks: vec![Block::Paragraph {
                    spans: vec![span("earlier")],
                }],
            },
        ],
    };
    let (mut phone, _) = open(0, vec![html_body("Q3 See our bank now", &document)]);
    let frame = phone.frame();
    let message = &reader(&frame).messages[0];
    let blocks = message.body.as_ref().unwrap();
    assert!(matches!(&blocks[0], DocBlock::Heading { level: 1, spans } if text(spans) == "Q3"));
    let DocBlock::Paragraph { spans } = &blocks[1] else {
        panic!("{blocks:?}")
    };
    assert_eq!(text(spans), "See our bank now");
    assert!(spans[2].bold);
    assert!(
        matches!(&blocks[2], DocBlock::List { ordered: true, start: 3, items } if items.len() == 1)
    );
    assert!(matches!(&blocks[3], DocBlock::Preformatted { lines } if lines == &["a  b", "c"]));
    assert!(matches!(&blocks[4], DocBlock::Table { rows } if rows[0][0].header));
    assert_eq!(blocks[5], DocBlock::Rule);
    assert!(matches!(&blocks[6], DocBlock::Quote { .. }));
    let DocBlock::Fold {
        label,
        expanded: false,
        toggle,
        blocks: folded,
    } = &blocks[7]
    else {
        panic!("{blocks:?}")
    };
    assert_eq!(label, "Quoted text");
    assert!(folded.is_empty(), "folded until tapped");
    assert_eq!(message.links.len(), 1, "Links (1)");
    assert_eq!(message.links[0].label, "https://evil.example/login");

    // The fold opens with a tap; no command runs.
    assert!(phone.tap(*toggle).is_empty());
    let frame = phone.frame();
    let blocks = reader(&frame).messages[0].body.clone().unwrap();
    assert!(
        matches!(&blocks[7], DocBlock::Fold { expanded: true, blocks, .. } if !blocks.is_empty())
    );

    // A link's tap shows its full target; nothing opens yet.
    let DocBlock::Paragraph { spans } = &blocks[1] else {
        panic!()
    };
    assert!(phone.tap(spans[1].link.unwrap()).is_empty());
    assert!(phone.mobile.externals().is_empty());
    let frame = phone.frame();
    let sheet = frame.link.as_ref().unwrap();
    assert_eq!(sheet.url, "https://evil.example/login");
    assert!(frame.back, "Back closes the sheet");
    phone.tap(sheet.open.action);
    assert_eq!(
        phone.mobile.externals(),
        [External::OpenUrl {
            url: "https://evil.example/login".to_owned()
        }]
    );
    assert!(phone.frame().link.is_none());

    // Copy, and Back closes the sheet without acting.
    let frame = phone.frame();
    let links = &reader(&frame).messages[0].links;
    phone.tap(links[0].action);
    let copy = phone.frame().link.unwrap().copy.action;
    phone.tap(copy);
    assert!(
        matches!(&phone.mobile.externals()[..], [External::Copy { text }] if text.contains("evil"))
    );
    let frame = phone.frame();
    phone.tap(reader(&frame).messages[0].links[0].action);
    assert!(phone.frame().link.is_some());
    phone.take();
    assert!(phone.event(UiEvent::Back).is_empty());
    assert!(phone.frame().link.is_none());
}

#[test]
fn a_mailto_link_opens_a_draft_not_an_app() {
    let document = Document {
        blocks: vec![Block::Paragraph {
            spans: vec![Span {
                text: "write".to_owned(),
                link: Some("mailto:bob@example.org?subject=Hi".to_owned()),
                ..Span::default()
            }],
        }],
    };
    let (mut phone, _) = open(0, vec![html_body("write", &document)]);
    let frame = phone.frame();
    let link = reader(&frame).messages[0].links[0].action;
    phone.tap(link);
    let open = phone.frame().link.unwrap().open.action;
    let got = phone.tap(open);
    assert!(
        matches!(got.first(), Some(Command::CreateDraft { subject, .. }) if subject == "Hi"),
        "{got:?}"
    );
    assert!(phone.mobile.externals().is_empty());
}

#[test]
fn plain_text_folds_its_signature_and_quote() {
    let body = "Sounds good.\n\nSee https://example.org/a.\n\n-- \nBob\n\nOn Mon, Alice wrote:\n> Lunch?\n";
    let (mut phone, _) = open(0, vec![plain_body(body)]);
    let frame = phone.frame();
    let message = &reader(&frame).messages[0];
    let blocks = message.body.as_ref().unwrap();
    let labels: Vec<&str> = blocks
        .iter()
        .filter_map(|b| match b {
            DocBlock::Fold { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(labels, ["Signature", "Quoted text"]);
    assert!(matches!(&blocks[0], DocBlock::Paragraph { spans } if text(spans) == "Sounds good."));
    assert_eq!(message.links[0].label, "https://example.org/a");
}

#[test]
fn search_words_are_marked_in_the_body() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::SetSearch {
        query: "lunch".to_owned(),
    });
    phone.bodies = Some(vec![plain_body("Lunch at noon? LUNCH!")]);
    phone.dispatch(Command::OpenThread { id: threads[0] });
    let frame = phone.frame();
    let Some(DocBlock::Paragraph { spans }) = reader(&frame).messages[0]
        .body
        .as_ref()
        .and_then(|b| b.first())
    else {
        panic!()
    };
    let hits: Vec<&str> = spans
        .iter()
        .filter(|s| s.hit)
        .map(|s| s.text.as_str())
        .collect();
    assert_eq!(hits, ["Lunch", "LUNCH"]);
}

#[test]
fn the_toolbar_toggles_star_and_read_and_walks_the_list() {
    let (mut phone, threads) = open(0, vec![plain_body("hi")]);
    let frame = phone.frame();
    let r = reader(&frame);
    assert_eq!(r.position.as_deref(), Some("1 of 2"));
    assert!(r.previous.is_none());
    let star = button(r, "Star");
    let read = r
        .actions
        .iter()
        .find(|b| b.label.starts_with("Mark "))
        .unwrap()
        .clone();
    let got = phone.tap(star);
    assert!(matches!(&got[..], [Command::Star { ids, .. }] if ids == &[threads[0]]));
    let frame = phone.frame();
    assert!(reader(&frame).actions.iter().any(|b| b.label == "Unstar"));
    let read = reader(&frame)
        .actions
        .iter()
        .find(|b| b.label == read.label)
        .unwrap()
        .action;
    let got = phone.tap(read);
    assert!(matches!(
        &got[..],
        [Command::MarkRead { .. } | Command::MarkUnread { .. }]
    ));
    let next = reader(&phone.frame()).next.clone().unwrap();
    assert_eq!(
        phone.tap(next.action),
        [Command::OpenThread { id: threads[1] }]
    );
    let frame = phone.frame();
    assert_eq!(reader(&frame).position.as_deref(), Some("2 of 2"));
    assert!(reader(&frame).next.is_none());
    assert!(reader(&frame).previous.is_some());
    for label in ["Archive", "Trash", "Move…", "Reply"] {
        button(reader(&frame), label);
    }
}

#[test]
fn back_closes_the_thread() {
    let (mut phone, _) = open(0, vec![plain_body("hi")]);
    let frame = phone.frame();
    assert!(frame.back);
    phone.take();
    let got = crate::support::requests_of(&phone.event(UiEvent::Back));
    assert_eq!(got, [Command::CloseThread {}]);
}
