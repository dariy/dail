//! TCH-2: the bars split their buttons into icons and a ⋮ menu by the window class.

use app_core::{Command, ThreadId};
use app_front::i18n;
use app_mobile::frame::{Button, Frame};

use crate::support::{Phone, SIZES, inbox_of};

fn labels(buttons: &[Button]) -> Vec<String> {
    buttons.iter().map(|b| b.label.clone()).collect()
}

/// The labels of the shared thread list for `ids`, in order.
fn shared(phone: &Phone, ids: &[ThreadId]) -> Vec<String> {
    app_front::actions::thread_actions(phone.state(), ids)
        .into_iter()
        .map(|e| i18n::english(e.label).to_owned())
        .collect()
}

/// The inbox at `width`×`height` with its first two threads checked.
fn checked(width: u32, height: u32) -> (Phone, Vec<ThreadId>) {
    let mut phone = Phone::inbox(width, height);
    let (_, threads) = inbox_of(phone.state());
    let ids = vec![threads[0], threads[1]];
    phone.dispatch(Command::CheckThreads { ids: ids.clone() });
    (phone, ids)
}

/// The inbox at `width`×`height` with its first thread open.
fn open(width: u32, height: u32) -> (Phone, ThreadId) {
    let mut phone = Phone::inbox(width, height);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    (phone, threads[0])
}

/// The budget the architecture documents for the reader of `frame`.
fn reader_budget(frame: &Frame) -> usize {
    use app_mobile::frame::{Class, PaneKind};
    let alone = frame.layout.panes == [PaneKind::Reader];
    match frame.layout.class {
        Class::Compact => 4,
        _ if alone => usize::MAX,
        Class::Medium => 4,
        Class::Expanded => 6,
    }
}

#[test]
fn a_phone_bulk_bar_has_four_icons_and_loses_nothing() {
    let (mut phone, ids) = checked(360, 800);
    let bulk = phone.frame().bulk.unwrap();
    assert_eq!(
        labels(&bulk.actions),
        ["Archive", "Trash", "Mark read", "Move…"]
    );
    assert_eq!(labels(&bulk.overflow), ["Junk", "Star", "Pin"]);
    let mut all = bulk.actions.clone();
    all.extend(bulk.overflow.clone());
    let mut order = labels(&all);
    // Both halves keep list order; merged by the shared list they are the whole list.
    let expected = shared(&phone, &ids);
    order.sort_by_key(|l| expected.iter().position(|e| e == l));
    assert_eq!(order, expected);
    assert!(bulk.overflow.iter().all(|b| !b.label.is_empty()));
}

#[test]
fn a_phone_reader_has_reply_archive_trash_and_move_as_icons() {
    let (mut phone, thread) = open(360, 800);
    let frame = phone.frame();
    let reader = frame.reader.unwrap();
    assert_eq!(
        labels(&reader.actions),
        ["Archive", "Trash", "Move…", "Reply"]
    );
    let mut expected = shared(&phone, &[thread]);
    expected.push("Reply".to_owned());
    let more = labels(&reader.overflow);
    assert_eq!(
        more,
        expected
            .iter()
            .filter(|l| !["Archive", "Trash", "Move…", "Reply"].contains(&l.as_str()))
            .cloned()
            .collect::<Vec<_>>()
    );
    assert!(more.iter().any(|l| l.starts_with("Mark ")), "{more:?}");
    assert!(reader.overflow.iter().all(|b| !b.label.is_empty()));
}

#[test]
fn a_phone_top_bar_has_at_most_four_icons() {
    let mut phone = Phone::inbox(360, 800);
    let bar = phone.frame().top_bar;
    assert!(bar.actions.len() <= 4);
    assert_eq!(labels(&bar.actions), ["Search or run a command"]);
    assert!(bar.overflow.is_empty());
}

#[test]
fn the_budgets_hold_at_every_size() {
    for (w, h) in SIZES {
        let (mut phone, ids) = checked(w, h);
        let frame = phone.frame();
        let bulk = frame.bulk.unwrap();
        let wide = w >= 600;
        assert!(wide || bulk.actions.len() <= 4, "{w}×{h}");
        assert!(!wide || bulk.overflow.is_empty(), "{w}×{h}");
        assert_eq!(
            bulk.actions.len() + bulk.overflow.len(),
            shared(&phone, &ids).len()
        );
        assert!(frame.top_bar.actions.len() <= 4 || wide);

        let (mut phone, thread) = open(w, h);
        let frame = phone.frame();
        let budget = reader_budget(&frame);
        let reader = frame.reader.unwrap();
        let total = shared(&phone, &[thread]).len() + 1;
        assert_eq!(reader.actions.len(), total.min(budget), "{w}×{h}");
        assert_eq!(reader.actions.len() + reader.overflow.len(), total);
    }
}

#[test]
fn an_overflow_entry_runs_what_its_icon_ran() {
    // *Star* is in the phone's menu and an icon on a wide window: the same command.
    let star = |w: u32, h: u32| {
        let (mut phone, _) = checked(w, h);
        let bulk = phone.frame().bulk.unwrap();
        let button = bulk
            .actions
            .iter()
            .chain(&bulk.overflow)
            .find(|b| b.label == "Star")
            .unwrap()
            .action;
        phone.tap(button)
    };
    let phone = star(360, 800);
    assert!(matches!(&phone[..], [Command::Star { ids, .. }] if ids.len() == 2));
    assert_eq!(phone, star(1280, 800));
    // *Move…* of the open message asks for the folder in the palette.
    let (mut phone, _) = open(360, 800);
    let reader = phone.frame().reader.unwrap();
    let mv = reader.actions.iter().find(|b| b.label == "Move…").unwrap();
    assert_eq!(phone.tap(mv.action), [Command::OpenPalette {}]);
    // The read toggle, in the menu, still toggles.
    let (mut phone, _) = open(360, 800);
    let reader = phone.frame().reader.unwrap();
    let read = reader
        .overflow
        .iter()
        .find(|b| b.label.starts_with("Mark "))
        .unwrap();
    assert!(matches!(
        &phone.tap(read.action)[..],
        [Command::MarkRead { .. } | Command::MarkUnread { .. }]
    ));
}
