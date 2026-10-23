//! Events → requests, dispatched with `Origin::Gui`.

use app_core::{Command, Pane};
use app_mobile::{External, UiEvent};

use crate::support::{Phone, inbox_of, requests_of};

#[test]
fn a_tap_on_a_row_opens_its_thread() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    let frame = phone.frame();
    let row = &frame.list.unwrap().rows[0];
    assert_eq!(row.thread, threads[0]);
    assert_eq!(
        phone.tap(row.action),
        [Command::OpenThread { id: threads[0] }]
    );
    let frame = phone.frame();
    assert_eq!(frame.reader.unwrap().thread, threads[0]);
    assert!(frame.back, "Back closes the thread");
    assert!(frame.list.is_none(), "one pane on a phone");
}

#[test]
fn stale_actions_are_ignored() {
    let mut phone = Phone::inbox(360, 800);
    let old = phone.frame().list.unwrap().rows[0].action;
    let _ = phone.frame();
    assert!(phone.tap(old).is_empty());
    assert!(phone.tap(0).is_empty());
    assert!(phone.tap(u32::MAX).is_empty());
}

#[test]
fn swipes_archive_and_trash_with_a_haptic() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    let rows = phone.frame().list.unwrap().rows;
    phone.event(UiEvent::Swipe {
        action: rows[0].swipe_start,
    });
    let sent = requests_of(&phone.take());
    assert_eq!(
        sent,
        [Command::ArchiveThreads {
            ids: vec![threads[0]],
            messages: Vec::new()
        }]
    );
    assert_eq!(phone.mobile.externals(), [External::Haptic]);
    let frame = phone.frame();
    let snackbar = frame.snackbar.unwrap();
    assert_eq!(snackbar.action.as_ref().unwrap().label, "Undo");
    assert_eq!(
        phone.tap(snackbar.action.unwrap().action),
        [Command::Undo {}]
    );
    let rows = phone.frame().list.unwrap().rows;
    assert_eq!(rows[0].thread, threads[0], "undo brought it back");
    phone.event(UiEvent::Swipe {
        action: rows[0].swipe_end,
    });
    assert_eq!(
        requests_of(&phone.take()),
        [Command::TrashThreads {
            ids: vec![threads[0]],
            messages: Vec::new()
        }]
    );
}

#[test]
fn a_long_press_checks_and_taps_then_toggle() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    let rows = phone.frame().list.unwrap().rows;
    phone.event(UiEvent::LongPress {
        action: rows[1].long_press,
    });
    assert_eq!(phone.state().view.checked, [threads[1]]);
    let frame = phone.frame();
    let list = frame.list.unwrap();
    assert_eq!(list.checked, 1);
    assert!(list.rows[1].checked);
    assert!(frame.back, "Back unchecks");
    assert_eq!(
        phone.tap(list.rows[0].action),
        [Command::CheckThreads {
            ids: vec![threads[0]]
        }]
    );
    let rows = phone.frame().list.unwrap().rows;
    phone.tap(rows[0].action);
    let second = phone.frame().list.unwrap().rows[1].action;
    phone.tap(second);
    assert!(phone.state().view.checked.is_empty());
}

#[test]
fn back_closes_the_thread_then_leaves_the_app() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    assert!(phone.frame().back);
    phone.event(UiEvent::Back);
    assert_eq!(phone.state().view.thread, None);
    assert!(!phone.frame().back, "Android takes Back now");
    phone.take();
    phone.event(UiEvent::Back);
    assert!(phone.take().is_empty());
}

#[test]
fn the_drawer_opens_from_the_menu_and_back_closes_it() {
    let mut phone = Phone::inbox(360, 800);
    let nav = phone.frame().top_bar.navigation.unwrap();
    assert_eq!(nav.icon.as_deref(), Some("menu"));
    assert_eq!(
        phone.tap(nav.action),
        [Command::FocusPane {
            pane: Pane::Sidebar
        }]
    );
    let frame = phone.frame();
    assert!(frame.back);
    let drawer = frame.drawer.unwrap();
    let sent = drawer.accounts[0]
        .mailboxes
        .iter()
        .find(|m| m.icon == "send")
        .unwrap()
        .action;
    phone.event(UiEvent::Back);
    assert_eq!(phone.state().view.focus, Pane::List);
    // The drawer's IDs died with its frame.
    let _ = phone.frame();
    assert!(phone.tap(sent).is_empty());
}

#[test]
fn compose_opens_a_new_draft() {
    let mut phone = Phone::inbox(360, 800);
    let fab = phone.frame().fab.unwrap();
    let sent = phone.tap(fab.action);
    assert!(matches!(sent[0], Command::CreateDraft { .. }));
    assert!(matches!(sent[1], Command::OpenComposer { .. }));
    let frame = phone.frame();
    assert!(frame.composer.is_some());
    assert!(frame.fab.is_none());
}

#[test]
fn refresh_and_keys() {
    let mut phone = Phone::inbox(360, 800);
    let _ = phone.frame();
    phone.event(UiEvent::Refresh);
    assert_eq!(
        requests_of(&phone.take()),
        [Command::SyncNow { account: None }]
    );
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    phone.event(UiEvent::Key {
        chord: "Escape".to_owned(),
    });
    assert_eq!(phone.state().view.thread, None);
    phone.take();
    phone.event(UiEvent::Key {
        chord: "not a chord".to_owned(),
    });
    assert!(phone.take().is_empty());
}

#[test]
fn bodies_load_for_the_open_thread() {
    let mut phone = Phone::inbox(700, 1000);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    let _ = phone.frame();
    let frame = phone.frame();
    let reader = frame.reader.unwrap();
    let expanded = reader.messages.iter().find(|m| m.expanded).unwrap();
    assert!(expanded.body.is_some() || expanded.status.is_some());
}

#[test]
fn the_bulk_bar_acts_on_the_checked_threads() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    assert!(phone.frame().bulk.is_none());
    let rows = phone.frame().list.unwrap().rows;
    phone.event(UiEvent::LongPress {
        action: rows[0].long_press,
    });
    assert_eq!(phone.mobile.externals(), [External::Haptic]);
    let rows = phone.frame().list.unwrap().rows;
    phone.tap(rows[1].action);
    let bulk = phone.frame().bulk.unwrap();
    assert_eq!(bulk.title, "2 selected");
    let labels: Vec<_> = bulk.actions.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(
        labels,
        ["Archive", "Trash", "Mark read", "Mark unread", "Star"]
    );
    let sent = phone.tap(bulk.actions[4].action);
    let ids = match &sent[0] {
        Command::Star { ids, .. } => ids.clone(),
        _ => Vec::new(),
    };
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&threads[0]) && ids.contains(&threads[1]));
    let close = phone.frame().bulk.unwrap().close;
    assert_eq!(
        phone.tap(close.action),
        [Command::UncheckThreads { ids: Vec::new() }]
    );
    assert!(phone.frame().bulk.is_none());
}
