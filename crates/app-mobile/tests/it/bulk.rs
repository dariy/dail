//! TCH-3: the bulk bar is `thread_actions` of the checked threads plus *Select all*.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{Command, MailboxRole, ThreadId};
use app_mobile::frame::{Bulk, Button};

use crate::support::{Phone, inbox_of, synced_state};

fn bulk(phone: &mut Phone) -> Bulk {
    phone.frame().bulk.unwrap()
}

fn buttons(bulk: &Bulk) -> Vec<Button> {
    bulk.actions.iter().chain(&bulk.overflow).cloned().collect()
}

fn labels(bulk: &Bulk) -> Vec<String> {
    buttons(bulk).into_iter().map(|b| b.label).collect()
}

fn find(bulk: &Bulk, label: &str) -> Button {
    buttons(bulk)
        .into_iter()
        .find(|b| b.label == label)
        .unwrap_or_else(|| panic!("no {label} in {:?}", labels(bulk)))
}

/// The phone's inbox with `which` of its threads checked, after `before` ran on all of them.
fn checked(
    which: &[usize],
    before: impl Fn(Vec<ThreadId>) -> Vec<Command>,
) -> (Phone, Vec<ThreadId>) {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    for cmd in before(threads.clone()) {
        phone.dispatch(cmd);
    }
    let ids: Vec<_> = which.iter().map(|&i| threads[i]).collect();
    phone.dispatch(Command::CheckThreads { ids: ids.clone() });
    (phone, ids)
}

fn count(labels: &[String], of: &[&str]) -> usize {
    labels.iter().filter(|l| of.contains(&l.as_str())).count()
}

#[test]
fn move_runs_move_threads_on_every_checked_thread() {
    let (mut phone, ids) = checked(&[0, 1], |_| Vec::new());
    let mv = find(&bulk(&mut phone), "Move…");
    assert_eq!(phone.tap(mv.action), [Command::OpenPalette {}]);
    let frame = phone.frame();
    let row = frame.palette.unwrap().sections[0].rows[0].clone();
    let sent = phone.tap(row.action);
    let moved = sent
        .iter()
        .find_map(|c| match c {
            Command::MoveThreads { ids, .. } => Some(ids.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{sent:?}"));
    assert_eq!(moved.len(), 2);
    assert!(ids.iter().all(|id| moved.contains(id)));
}

#[test]
fn junk_or_not_junk_by_folder() {
    let (mut phone, _) = checked(&[0, 1], |_| Vec::new());
    let all = labels(&bulk(&mut phone));
    assert_eq!((count(&all, &["Junk"]), count(&all, &["Not junk"])), (1, 0));

    let mut state = synced_state();
    state.settings.stale_secs = 86_400;
    let (inbox, threads) = inbox_of(&state);
    let mut folder = state.mailboxes[&inbox].clone();
    folder.id = app_core::IdGen::new(78).next_id();
    folder.path = "Junk".into();
    folder.role = Some(MailboxRole::Junk);
    let junk = folder.id;
    state.mailboxes.insert(junk, folder);
    let mut phone = Phone::new(state, 360, 800);
    // The thread is in Junk: only a thread the list shows stays checked.
    phone.dispatch(Command::MarkJunk {
        ids: vec![threads[0]],
        messages: Vec::new(),
    });
    phone.dispatch(Command::OpenMailbox { id: junk });
    phone.dispatch(Command::CheckThreads {
        ids: vec![threads[0]],
    });
    let all = labels(&bulk(&mut phone));
    assert_eq!((count(&all, &["Junk"]), count(&all, &["Not junk"])), (0, 1));
}

#[test]
fn one_read_and_one_star_toggle_follow_the_checked_threads() {
    let toggles = |before: fn(Vec<ThreadId>) -> Vec<Command>| {
        let (mut phone, _) = checked(&[0, 1], before);
        let all = labels(&bulk(&mut phone));
        assert_eq!(count(&all, &["Mark read", "Mark unread"]), 1, "{all:?}");
        assert_eq!(count(&all, &["Star", "Unstar"]), 1, "{all:?}");
        let read = all.iter().find(|l| l.starts_with("Mark ")).unwrap().clone();
        let star = all.iter().find(|l| l.ends_with("tar")).unwrap().clone();
        (read, star)
    };
    let mixed = toggles(|t| {
        vec![
            Command::MarkRead {
                ids: vec![t[0]],
                messages: Vec::new(),
            },
            Command::MarkUnread {
                ids: vec![t[1]],
                messages: Vec::new(),
            },
            Command::Star {
                ids: vec![t[0]],
                messages: Vec::new(),
            },
        ]
    });
    assert_eq!(mixed, ("Mark read".to_owned(), "Star".to_owned()));
    let all = toggles(|t| {
        vec![
            Command::MarkRead {
                ids: t.clone(),
                messages: Vec::new(),
            },
            Command::Star {
                ids: t,
                messages: Vec::new(),
            },
        ]
    });
    assert_eq!(all, ("Mark unread".to_owned(), "Unstar".to_owned()));
}

#[test]
fn select_all_checks_every_shown_thread_until_all_are() {
    let (mut phone, _) = checked(&[0], |_| Vec::new());
    let bar = bulk(&mut phone);
    let select = find(&bar, "Select all");
    // In the phone's ⋯ menu, after the thread actions and before the palette.
    assert_eq!(select.icon.as_deref(), Some("check_box"));
    let more: Vec<_> = bar.overflow.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(
        more[more.len() - 2..],
        ["Select all", "Search or run a command"]
    );
    assert_eq!(
        phone.tap(select.action),
        [Command::CheckThreads { ids: Vec::new() }]
    );
    let shown = app_core::listing::visible_threads(phone.state());
    assert!(
        shown
            .iter()
            .all(|id| phone.state().view.checked.contains(id))
    );
    // Every shown thread is checked: nothing left to select (close unchecks all).
    let bar = bulk(&mut phone);
    assert!(!labels(&bar).contains(&"Select all".to_owned()));
}

/// At 320 dp four icons, Close and the ⋯ menu left the title 32 dp: three icons there.
#[test]
fn a_narrow_phone_draws_three_icons_so_the_title_fits() {
    let mut phone = Phone::inbox(320, 640);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::CheckThreads {
        ids: vec![threads[0], threads[1]],
    });
    let bar = bulk(&mut phone);
    assert_eq!(bar.title, "2 selected");
    assert_eq!(bar.actions.len(), 3, "{:?}", labels(&bar));
    let mut phone = Phone::inbox(360, 640);
    phone.dispatch(Command::CheckThreads {
        ids: vec![threads[0], threads[1]],
    });
    assert_eq!(bulk(&mut phone).actions.len(), 4);
}
