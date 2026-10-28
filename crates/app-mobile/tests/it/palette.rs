//! The palette sheet: the top bar's search opens it, rows under captions, second steps, a
//! search with marks and the server's state, a query's result, and the parity walk of every
//! registry command by taps.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{Command, CommandName};
use app_front::palette::offered;
use app_mobile::event::Field;
use app_mobile::frame::PaletteSheet;
use app_mobile::{Frame, UiEvent};

use crate::support::{Phone, inbox_of, requests_of};

fn sheet(frame: &Frame) -> &PaletteSheet {
    frame.palette.as_ref().unwrap()
}

/// Every row of the sheet, in order.
fn rows(frame: &Frame) -> Vec<(String, u32)> {
    sheet(frame)
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .map(|r| (r.title.clone(), r.action))
        .collect()
}

fn search_button(frame: &Frame) -> u32 {
    frame
        .top_bar
        .actions
        .iter()
        .find(|b| b.icon.as_deref() == Some("search"))
        .unwrap()
        .action
}

fn typed(phone: &mut Phone, text: &str) {
    phone.event(UiEvent::Text {
        field: Field::Query,
        text: text.to_owned(),
    });
}

#[test]
fn the_search_button_opens_the_palette_with_captions() {
    let mut phone = Phone::inbox(360, 800);
    let frame = phone.frame();
    assert_eq!(phone.tap(search_button(&frame)), [Command::OpenPalette {}]);
    let frame = phone.frame();
    assert!(frame.back, "Back closes the palette");
    let titles: Vec<&str> = sheet(&frame)
        .sections
        .iter()
        .map(|s| s.title.as_str())
        .collect();
    for caption in ["Mail", "Mailboxes", "Threads"] {
        assert!(titles.contains(&caption), "{caption} in {titles:?}");
    }
    assert_eq!(sheet(&frame).hint, "Type a command or search…");
    assert_eq!(
        phone.event(UiEvent::Back),
        [Command::ClosePalette {}].map(app_mobile::Request::user)
    );
    assert!(phone.frame().palette.is_none());
}

#[test]
fn typed_text_ranks_the_rows_and_offers_a_search() {
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::OpenPalette {});
    typed(&mut phone, "lunch");
    let frame = phone.frame();
    let rows = rows(&frame);
    assert_eq!(sheet(&frame).sections.len(), 1, "one ranked list");
    assert!(rows.iter().any(|(t, _)| t == "Lunch?"), "{rows:?}");
    let (_, search) = rows.last().unwrap().clone();
    assert_eq!(rows.last().unwrap().0, "Search mail for “lunch”");
    assert_eq!(
        phone.tap(search),
        [
            Command::SetSearch {
                query: "lunch".to_owned()
            },
            Command::ClosePalette {}
        ]
    );
    let frame = phone.frame();
    assert!(frame.palette.is_none());
    let list = frame.list.unwrap();
    assert_eq!(list.title, "Search: lunch");
    let row = &list.rows[0];
    let marks = row.marks.as_ref().expect("marked while searching");
    assert!(
        marks.subject.iter().any(|p| p.hit && p.text == "Lunch"),
        "{marks:?}"
    );
    assert_eq!(
        marks
            .subject
            .iter()
            .map(|p| p.text.as_str())
            .collect::<String>(),
        row.subject
    );
}

#[test]
fn move_opens_the_palette_at_its_step() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    let frame = phone.frame();
    let reader = frame.reader.unwrap();
    let mv = reader.actions.iter().find(|b| b.label == "Move…").unwrap();
    assert_eq!(phone.tap(mv.action), [Command::OpenPalette {}]);
    let frame = phone.frame();
    let sheet = sheet(&frame);
    assert!(sheet.prompt.is_some(), "a step");
    assert_eq!(sheet.hint, "Filter…");
    let version = sheet.query.version;
    let archive = rows(&frame)
        .into_iter()
        .find(|(t, _)| t.contains("Archive"))
        .unwrap();
    let got = phone.tap(archive.1);
    assert!(
        matches!(got.first(), Some(Command::MoveThreads { ids, .. }) if ids == &[threads[0]]),
        "{got:?}"
    );
    assert_eq!(got.last(), Some(&Command::ClosePalette {}));
    // A later palette starts afresh, with a new query version.
    phone.dispatch(Command::OpenPalette {});
    let frame = phone.frame();
    assert!(self::sheet(&frame).prompt.is_none());
    assert!(self::sheet(&frame).query.version > version);
}

#[test]
fn back_leaves_a_step_before_closing() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    phone.dispatch(Command::OpenPalette {});
    typed(&mut phone, "move to");
    let frame = phone.frame();
    let (_, first) = rows(&frame)[0].clone();
    assert!(phone.tap(first).is_empty(), "asks for the mailbox");
    assert!(sheet(&phone.frame()).prompt.is_some());
    phone.take();
    assert!(
        phone.event(UiEvent::Back).is_empty(),
        "back to the commands"
    );
    let frame = phone.frame();
    assert!(sheet(&frame).prompt.is_none());
    assert_eq!(sheet(&frame).query.text, "");
    phone.take();
    assert_eq!(
        requests_of(&phone.event(UiEvent::Back)),
        [Command::ClosePalette {}]
    );
}

#[test]
fn a_query_shows_its_result_read_only() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    phone.dispatch(Command::OpenPalette {});
    typed(&mut phone, CommandName::ListMailboxes.spec().title);
    let frame = phone.frame();
    let (_, first) = rows(&frame)[0].clone();
    let got = phone.tap(first);
    assert_eq!(
        got.first().map(Command::command_name),
        Some(CommandName::ListMailboxes)
    );
    let frame = phone.frame();
    let result = frame.result.as_ref().expect("the result shows");
    assert!(result.note.contains("untrusted"));
    // A view, not JSON (MISC-22): the mailboxes as rows that open them.
    assert!(result.lines.is_empty());
    let rows: Vec<_> = result.sections.iter().flat_map(|s| &s.rows).collect();
    assert!(rows.iter().any(|r| r.title == "Inbox"), "{rows:?}");
    // *Raw data* shows the JSON, *View* goes back.
    let raw = result.raw.action;
    assert!(phone.tap(raw).is_empty());
    let frame = phone.frame();
    let result = frame.result.as_ref().unwrap();
    assert!(result.sections.is_empty());
    assert!(
        result.lines.iter().any(|l| l.contains("INBOX")),
        "{:?}",
        result.lines
    );
    assert_eq!(result.raw.label, "View");
    assert!(frame.back);
    phone.take();
    assert!(phone.event(UiEvent::Back).is_empty());
    assert!(phone.frame().result.is_none());
}

#[test]
fn a_result_row_opens_its_mailbox() {
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::OpenPalette {});
    typed(&mut phone, CommandName::ListMailboxes.spec().title);
    let frame = phone.frame();
    let (_, first) = rows(&frame)[0].clone();
    phone.tap(first);
    let frame = phone.frame();
    let result = frame.result.as_ref().unwrap();
    let row = result
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .find(|r| r.title == "Sent")
        .unwrap();
    let got = phone.tap(row.action.unwrap());
    assert_eq!(
        got.first().map(Command::command_name),
        Some(CommandName::OpenMailbox)
    );
    assert!(phone.frame().result.is_none());
}

#[test]
fn every_registry_command_is_reachable_by_taps() {
    for &name in CommandName::ALL {
        let mut phone = crate::support::rich_phone();
        if !offered(name) || !(name.spec().available)(phone.state()) {
            continue;
        }
        let frame = phone.frame();
        phone.tap(search_button(&frame));
        typed(&mut phone, name.spec().title);
        let mut got = Vec::new();
        for _ in 0..6 {
            let frame = phone.frame();
            let Some(sheet) = frame.palette.as_ref() else {
                break;
            };
            if sheet.prompt.is_some() && sheet.hint != "Filter…" {
                // A free-text step.
                let text = if matches!(name, CommandName::SetSetting | CommandName::SetSyncWindow) {
                    "300"
                } else {
                    "plans"
                };
                typed(&mut phone, text);
            }
            let frame = phone.frame();
            let Some((_, action)) = rows(&frame).first().cloned() else {
                break;
            };
            got.extend(phone.tap(action));
            if !got.is_empty() {
                break;
            }
        }
        match name {
            CommandName::AddAccount | CommandName::SetPassword => {
                assert_eq!(got, [Command::ClosePalette {}], "{name:?}");
                let setup = phone.frame().setup.expect("the account setup opens");
                let title = if name == CommandName::AddAccount {
                    "Add an account"
                } else {
                    "Change the password"
                };
                assert_eq!(setup.title, title, "{name:?}");
            }
            name => {
                let expected = match name {
                    CommandName::UpdateDraft => CommandName::OpenComposer,
                    name => name,
                };
                let built = got.first().map(Command::command_name);
                assert_eq!(built, Some(expected), "{name:?} built {got:?}");
            }
        }
    }
}

#[test]
fn the_list_says_what_the_server_search_does() {
    let mut phone = Phone::inbox(360, 800);
    let (inbox, _) = inbox_of(phone.state());
    let account = *phone.state().accounts.keys().next().unwrap();
    phone.dispatch(Command::SetSearch {
        query: "lunch".to_owned(),
    });
    let failed = |error: &str| app_core::Event::SearchFailed {
        account,
        mailbox: inbox,
        search: "lunch".to_owned(),
        started_ms: crate::support::NOW,
        mailboxes: 2,
        error: error.to_owned(),
    };
    // One mailbox answered: the round still runs.
    phone.ingest(failed("busy"));
    let list = phone.frame().list.unwrap();
    assert_eq!(list.server.as_deref(), Some("Searching the server…"));
    // `set_search` started a round over every mailbox; the others fail too.
    while app_front::status::searching(phone.state()) {
        phone.ingest(failed("\u{1b}[2JNO SEARCH"));
    }
    let server = phone.frame().list.unwrap().server.unwrap();
    assert!(server.starts_with("The server search failed"), "{server}");
    assert!(!server.contains('\u{1b}'));
    phone.dispatch(Command::ClearSearch {});
    assert_eq!(phone.frame().list.unwrap().server, None);
}
