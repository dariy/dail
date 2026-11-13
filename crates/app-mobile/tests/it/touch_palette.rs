//! TCH-1: the palette opens from every touch state with a bar, through the bar's overflow where
//! the top bar's search is not on screen, and never from two buttons at once.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{Command, SettingsSection};
use app_mobile::event::Field;
use app_mobile::frame::{Button, PaneKind};
use app_mobile::{Frame, UiEvent};

use crate::support::{Phone, SIZES, inbox_of};

/// Every button of the bars the frame draws. The top bar is hidden by a bulk bar and by a
/// phone's reader page.
fn bar_buttons(frame: &Frame) -> Vec<Button> {
    let mut out = Vec::new();
    if let Some(bulk) = &frame.bulk {
        out.extend(bulk.actions.iter().chain(&bulk.overflow).cloned());
    } else if frame.layout.page != Some(PaneKind::Reader) {
        let bar = &frame.top_bar;
        out.extend(bar.actions.iter().chain(&bar.overflow).cloned());
    }
    if let Some(reader) = &frame.reader {
        out.extend(reader.actions.iter().chain(&reader.overflow).cloned());
    }
    if let Some(composer) = &frame.composer {
        out.extend(composer.actions.iter().cloned());
    }
    out
}

/// How many buttons of the bars of the `w`×`h` state built by `setup` open the palette at its
/// top (a flow's button opens it at a step). Each
/// tap is made on a fresh copy of the state, as a tap changes it.
fn count(w: u32, h: u32, setup: impl Fn(&mut Phone)) -> usize {
    let build = || {
        let mut phone = Phone::inbox(w, h);
        setup(&mut phone);
        phone
    };
    let total = bar_buttons(&build().frame()).len();
    (0..total)
        .filter(|&at| {
            let mut phone = build();
            let button = bar_buttons(&phone.frame())[at].clone();
            // *Move…* and *Make a rule…* open the palette too, at their step: the entry
            // that opens it at the top is the `search` one.
            button.icon.as_deref() == Some("search")
                && phone.tap(button.action) == [Command::OpenPalette {}]
        })
        .count()
}

fn open_first(phone: &mut Phone) {
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
}

fn check_two(phone: &mut Phone) {
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::CheckThreads {
        ids: vec![threads[0], threads[1]],
    });
}

/// The open message, maximized where there is anything to hide.
fn maximized(phone: &mut Phone) {
    open_first(phone);
    if let Some(maximize) = phone.frame().reader.unwrap().maximize {
        phone.tap(maximize.action);
    }
}

fn composing(phone: &mut Phone) {
    let fab = phone.frame().fab.unwrap();
    phone.tap(fab.action);
}

fn settings(phone: &mut Phone) {
    phone.dispatch(Command::OpenSettings {
        section: Some(SettingsSection::General),
    });
}

#[test]
fn every_touch_state_reaches_the_palette_once() {
    for (w, h) in SIZES {
        assert_eq!(count(w, h, |_| {}), 1, "inbox at {w}×{h}");
        assert_eq!(count(w, h, open_first), 1, "message at {w}×{h}");
        assert_eq!(count(w, h, check_two), 1, "bulk at {w}×{h}");
        assert_eq!(count(w, h, maximized), 1, "maximized at {w}×{h}");
        assert_eq!(count(w, h, composing), 1, "composer at {w}×{h}");
        assert_eq!(count(w, h, settings), 1, "settings at {w}×{h}");
    }
}

#[test]
fn a_maximized_message_carries_the_palette() {
    let mut phone = Phone::inbox(1280, 800);
    maximized(&mut phone);
    let frame = phone.frame();
    assert!(
        frame.top_bar.actions.is_empty(),
        "a maximized message clears the top bar"
    );
    let reader = frame.reader.unwrap();
    assert!(
        reader
            .actions
            .iter()
            .chain(&reader.overflow)
            .any(|b| b.icon.as_deref() == Some("search"))
    );
}

#[test]
fn the_palette_from_the_bulk_bar_acts_on_the_checked_threads() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    check_two(&mut phone);
    let bulk = phone.frame().bulk.unwrap();
    let entry = bulk
        .overflow
        .iter()
        .find(|b| b.icon.as_deref() == Some("search"))
        .unwrap();
    assert_eq!(phone.tap(entry.action), [Command::OpenPalette {}]);
    phone.event(UiEvent::Text {
        field: Field::Query,
        text: "star".to_owned(),
    });
    let frame = phone.frame();
    let row = frame
        .palette
        .unwrap()
        .sections
        .iter()
        .flat_map(|s| s.rows.clone())
        .find(|r| r.title.starts_with("Star"))
        .unwrap();
    let sent = phone.tap(row.action);
    let ids = match &sent[..] {
        [Command::Star { ids, .. }, ..] => ids.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&threads[0]) && ids.contains(&threads[1]));
}
