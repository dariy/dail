//! Email content cannot spoof the app: a subject, names, addresses, bodies, link texts and
//! targets, attachment names and Content-IDs carrying `ESC ]52;…` (set the clipboard), `ESC [2J`,
//! C1 controls, `\r` and U+202E (right-to-left override) leave none of them in any string of any
//! frame (line breaks stay only where a body has them), at every window class: the list, the
//! drawer, the reader, a neighbour's peek, the link dialog, a preview, the palette, a query's result
//! and a reply's composer. Only U+FFFD shows where they were.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::document::{Block, Document, Span};
use app_core::{Addr, BodyState, Command, CommandName, ThreadId};
use app_front::text::is_unsafe;
use app_mobile::event::Field;
use app_mobile::{Frame, UiEvent};

use crate::support::{Phone, SIZES, html_body, inbox_of, plain_body, synced_state};

const NASTY: &str = "\u{1b}]52;c;ZXZpbA==\u{7}Pay\u{1b}[2Jnow\r\u{202e}gpj.exe\u{9b}31m";

/// The synced fixture at `width`×`height` with every attacker-controlled field of the inbox's
/// newest thread set to [`NASTY`], its first message fetched, and its bodies poisoned too.
fn poisoned(width: u32, height: u32) -> (Phone, ThreadId) {
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let thread = threads[0];
    state.threads.get_mut(&thread).unwrap().subject = NASTY.to_owned();
    let nasty = Addr {
        name: Some(NASTY.to_owned()),
        email: format!("bob{NASTY}@example.org"),
    };
    for (i, message) in state.threads[&thread]
        .messages
        .clone()
        .into_iter()
        .enumerate()
    {
        let envelope = state.messages.get_mut(&message).unwrap();
        envelope.subject = NASTY.to_owned();
        envelope.snippet = Some(NASTY.to_owned());
        envelope.from = vec![nasty.clone()];
        envelope.to.push(nasty.clone());
        envelope.cc = vec![nasty.clone()];
        if i == 0 {
            envelope.body = BodyState::Fetched {
                blob: "b0".to_owned(),
            };
        }
    }
    if let Some(folder) = state.mailboxes.values_mut().find(|m| m.role.is_none()) {
        folder.path = NASTY.to_owned();
    }
    state
        .settings
        .images_from
        .insert(nasty.email.to_lowercase());
    let mut phone = Phone::new(state, width, height);
    phone.dispatch(Command::OpenMailbox { id: inbox });
    let document = Document {
        blocks: vec![
            Block::Heading {
                level: 1,
                spans: vec![text(NASTY)],
            },
            Block::Paragraph {
                spans: vec![
                    Span {
                        text: NASTY.to_owned(),
                        link: Some(format!("https://example.org/{NASTY}")),
                        ..Span::default()
                    },
                    Span {
                        text: NASTY.to_owned(),
                        image: Some(NASTY.to_owned()),
                        ..Span::default()
                    },
                ],
            },
            Block::Preformatted {
                text: NASTY.to_owned(),
            },
            Block::Table {
                rows: vec![vec![app_core::document::Cell {
                    header: true,
                    spans: vec![text(NASTY)],
                }]],
            },
        ],
    };
    let mut html = html_body(NASTY, &document);
    html["attachments"] = serde_json::json!([
        {"part": 2, "filename": NASTY, "content_type": format!("text/plain{NASTY}"), "size": 1},
        {"part": 3, "filename": NASTY, "content_type": "application/pdf", "size": 1},
    ]);
    phone.bodies = Some(vec![
        html,
        plain_body(&format!(
            "{NASTY}\nsee https://example.org/{NASTY} \n-- \n{NASTY}"
        )),
    ]);
    (phone, thread)
}

fn text(text: &str) -> Span {
    Span {
        text: text.to_owned(),
        ..Span::default()
    }
}

/// Every string of `value`, with its path.
fn strings(value: &serde_json::Value, path: &str, out: &mut Vec<(String, String)>) {
    match value {
        serde_json::Value::String(s) => out.push((path.to_owned(), s.clone())),
        serde_json::Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                strings(item, &format!("{path}[{i}]"), out);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                strings(item, &format!("{path}.{key}"), out);
            }
        }
        _ => {}
    }
}

/// Checks that no string of `frame` carries an unsafe character (but a body's line breaks),
/// and that the poisoned text did reach it, replaced.
fn assert_clean(frame: &Frame, what: &str) {
    let json = serde_json::to_value(frame).unwrap();
    let mut all = Vec::new();
    strings(&json, "frame", &mut all);
    for (path, s) in &all {
        let bad: Vec<char> = s.chars().filter(|&c| c != '\n' && is_unsafe(c)).collect();
        assert!(bad.is_empty(), "{what}: {bad:?} at {path}: {s:?}");
        if s.contains('\n') {
            assert!(
                path.contains(".body[") || path.contains(".lines["),
                "{what}: a line break outside a body at {path}: {s:?}"
            );
        }
    }
    assert!(
        all.iter().any(|(_, s)| s.contains('\u{fffd}')),
        "{what} shows no poisoned text"
    );
}

#[test]
fn no_frame_carries_unsafe_characters() {
    for (width, height) in SIZES {
        let (mut phone, thread) = poisoned(width, height);
        assert_clean(&phone.frame(), "the list");

        phone.dispatch(Command::FocusPane {
            pane: app_core::Pane::Sidebar,
        });
        assert_clean(&phone.frame(), "the drawer");
        phone.dispatch(Command::FocusPane {
            pane: app_core::Pane::List,
        });

        phone.dispatch(Command::OpenThread { id: thread });
        let ids = phone.state().threads[&thread].messages.clone();
        phone.dispatch(Command::ExpandMessages { ids });
        let frame = phone.frame();
        assert_clean(&frame, "the reader");
        let message = frame.reader.as_ref().unwrap().messages[0].clone();
        assert!(!message.attachments.is_empty() && !message.links.is_empty());

        phone.tap(message.links[0].action);
        let frame = phone.frame();
        assert!(frame.link.is_some());
        assert_clean(&frame, "the link dialog");
        phone.event(UiEvent::Back);

        let frame = phone.frame();
        let open = frame.reader.as_ref().unwrap().messages[0].attachments[0].open;
        phone.tap(open);
        let frame = phone.frame();
        assert!(frame.preview.is_some());
        assert_clean(&frame, "a preview");
        phone.event(UiEvent::Back);

        // The poisoned thread as the peek of the thread beside it (MISC-18).
        let rows: Vec<ThreadId> = phone
            .frame()
            .list
            .unwrap()
            .rows
            .iter()
            .map(|r| r.thread)
            .collect();
        let at = rows.iter().position(|t| *t == thread).unwrap();
        let beside = if at > 0 { rows[at - 1] } else { rows[at + 1] };
        phone.dispatch(Command::OpenThread { id: beside });
        let frame = phone.frame();
        let reader = frame.reader.as_ref().unwrap();
        let peek = if at > 0 {
            &reader.next
        } else {
            &reader.previous
        };
        assert!(
            peek.as_ref().unwrap().subject.contains('\u{fffd}'),
            "the poisoned peek"
        );
        assert_clean(&frame, "a neighbour's peek");
        phone.dispatch(Command::OpenThread { id: thread });

        // The poisoned thread among the palette's recent threads.
        phone.dispatch(Command::OpenPalette {});
        phone.event(UiEvent::Text {
            field: Field::Query,
            text: "Pay".to_owned(),
        });
        assert_clean(&phone.frame(), "the palette");
        phone.event(UiEvent::Text {
            field: Field::Query,
            text: CommandName::GetThread.spec().title.to_owned(),
        });
        let row = phone.frame().palette.unwrap().sections[0].rows[0].action;
        phone.tap(row);
        // The step (a thread) or the result directly.
        if let Some(sheet) = phone.frame().palette {
            let row = sheet.sections[0].rows[0].action;
            phone.tap(row);
        }
        let frame = phone.frame();
        assert!(frame.result.is_some(), "a query's result");
        assert_clean(&frame, "a query's result");
        phone.event(UiEvent::Back);

        let frame = phone.frame();
        let reply = frame
            .reader
            .as_ref()
            .unwrap()
            .actions
            .iter()
            .find(|b| b.label == "Reply");
        phone.tap(reply.unwrap().action);
        let frame = phone.frame();
        assert!(frame.composer.is_some());
        assert_clean(&frame, "a reply");
    }
}
