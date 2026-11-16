//! The action lists of threads, mailboxes and accounts (PAR-1).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{AccountId, AppState, Command, MailboxId, MailboxRole, ThreadId};
use app_front::actions::{Entry, Run, account_actions, mailbox_actions, thread_actions};
use app_front::i18n::Key;
use app_front::palette::{self, AskKind, Choice, Flow, Target};

use crate::support::{inbox_of, synced_state};

/// (label, icon, primary) of each entry.
fn shape(entries: &[Entry]) -> Vec<(Key, &'static str, bool)> {
    entries
        .iter()
        .map(|e| (e.label, e.icon, e.primary))
        .collect()
}

fn set_flags(state: &mut AppState, thread: ThreadId, seen: bool, flagged: bool, pinned: bool) {
    let messages = state.threads[&thread].messages.clone();
    for m in messages {
        let flags = &mut state.messages[&m].flags;
        flags.seen = seen;
        flags.flagged = flagged;
        flags.pinned = pinned;
    }
}

/// A copy of the inbox with `role` and `path`.
fn add_folder(
    state: &mut AppState,
    seed: u128,
    path: &str,
    role: Option<MailboxRole>,
) -> MailboxId {
    let (inbox, _) = inbox_of(state);
    let mut folder = state.mailboxes[&inbox].clone();
    folder.id = app_core::IdGen::new(seed).next_id();
    folder.path = path.into();
    folder.role = role;
    let id = folder.id;
    state.mailboxes.insert(id, folder);
    id
}

/// Follows a flow, answering every step with its first choice (`text` for free text).
fn walk(state: &AppState, mut flow: Flow, text: &str) -> Flow {
    for _ in 0..6 {
        let Flow::Ask(step) = &flow else {
            return flow;
        };
        let choice = match &step.kind {
            AskKind::Text(_) => Choice::Text(text.to_owned()),
            AskKind::Confirm(_) => Choice::Yes,
            kind => match palette::ask_rows(state, kind, "")
                .first()
                .map(|r| r.target.clone())
            {
                Some(Target::Choice(choice)) => choice,
                other => panic!("{kind:?} offered {other:?}"),
            },
        };
        flow = step.answer(state, choice);
    }
    flow
}

fn ends_in(state: &AppState, run: &Run, text: &str) -> Command {
    match walk(state, run.flow(state), text) {
        Flow::Run(cmd) => cmd,
        other => panic!("{run:?} ended in {other:?}"),
    }
}

#[test]
fn one_unread_thread_offers_the_triage_move_and_a_rule() {
    let mut state = synced_state();
    let (_, threads) = inbox_of(&state);
    let t = threads[0];
    set_flags(&mut state, t, false, false, false);
    let list = thread_actions(&state, &[t]);
    assert_eq!(
        shape(&list),
        [
            (Key::Archive, "archive", true),
            (Key::Trash, "delete", true),
            (Key::MarkJunk, "report", false),
            (Key::MarkRead, "mark_email_read", true),
            (Key::Star, "star", false),
            (Key::Pin, "push_pin", false),
            (Key::Move, "drive_file_move", true),
            (Key::MakeRule, "edit", false),
        ]
    );
    let ids = vec![t];
    let runs: Vec<Run> = list.iter().map(|e| e.run.clone()).collect();
    let message = palette::newest_message(&state, t).unwrap();
    assert_eq!(
        runs,
        [
            Run::Command(Command::ArchiveThreads {
                ids: ids.clone(),
                messages: Vec::new()
            }),
            Run::Command(Command::TrashThreads {
                ids: ids.clone(),
                messages: Vec::new()
            }),
            Run::Command(Command::MarkJunk {
                ids: ids.clone(),
                messages: Vec::new()
            }),
            Run::Command(Command::MarkRead {
                ids: ids.clone(),
                messages: Vec::new()
            }),
            Run::Command(Command::Star {
                ids: ids.clone(),
                messages: Vec::new()
            }),
            Run::Command(Command::PinThreads {
                ids: ids.clone(),
                messages: Vec::new()
            }),
            Run::Move(ids.clone()),
            Run::RuleFrom(message),
        ]
    );
    assert!(list.iter().filter(|e| e.primary).count() <= 4);
    assert!(matches!(
        ends_in(&state, &runs[6], ""),
        Command::MoveThreads { ids: ref moved, .. } if *moved == ids
    ));
    assert!(matches!(
        ends_in(&state, &runs[7], ""),
        Command::SaveRule { .. }
    ));
}

#[test]
fn many_threads_toggle_by_their_state_and_make_no_rule() {
    let mut state = synced_state();
    let (_, threads) = inbox_of(&state);
    let (a, b) = (threads[0], threads[1]);
    // Both read, both starred, both pinned: the toggles undo.
    set_flags(&mut state, a, true, true, true);
    set_flags(&mut state, b, true, true, true);
    let labels = |state: &AppState| -> Vec<Key> {
        thread_actions(state, &[a, b])
            .iter()
            .map(|e| e.label)
            .collect()
    };
    assert_eq!(
        labels(&state),
        [
            Key::Archive,
            Key::Trash,
            Key::MarkJunk,
            Key::MarkUnread,
            Key::Unstar,
            Key::Unpin,
            Key::Move,
        ]
    );
    // One unread, unstarred and unpinned: mark read, star, pin.
    set_flags(&mut state, b, false, false, false);
    assert_eq!(
        labels(&state),
        [
            Key::Archive,
            Key::Trash,
            Key::MarkJunk,
            Key::MarkRead,
            Key::Star,
            Key::Pin,
            Key::Move,
        ]
    );
    let list = thread_actions(&state, &[a, b]);
    assert_eq!(list[6].run, Run::Move(vec![a, b]));
    assert_eq!(
        list[4].run,
        Run::Command(Command::Star {
            ids: vec![a, b],
            messages: Vec::new()
        })
    );
    assert!(thread_actions(&state, &[]).is_empty());
}

#[test]
fn in_a_junk_folder_a_thread_is_not_junk() {
    let mut state = synced_state();
    let (_, threads) = inbox_of(&state);
    let junk = add_folder(&mut state, 78, "Junk", Some(MailboxRole::Junk));
    state.view.mailbox = Some(junk);
    let list = thread_actions(&state, &[threads[0]]);
    assert_eq!((list[2].label, list[2].icon), (Key::NotJunk, "inbox"));
    assert_eq!(
        list[2].run,
        Run::Command(Command::MarkNotJunk {
            ids: vec![threads[0]],
            messages: Vec::new()
        })
    );
}

fn mailbox_shape(state: &AppState, id: MailboxId) -> Vec<(Key, &'static str, bool)> {
    shape(&mailbox_actions(state, id))
}

#[test]
fn the_inbox_cannot_be_renamed_or_deleted_or_emptied() {
    let state = synced_state();
    let (inbox, _) = inbox_of(&state);
    assert_eq!(
        mailbox_shape(&state, inbox),
        [
            (Key::NewSubfolder, "add", true),
            (Key::PinFavourites, "push_pin", true),
            (Key::HideFolder, "visibility_off", false),
            (Key::FocusMailbox, "open_in_full", false),
            (Key::MarkAllRead, "mark_email_read", true),
            (Key::ApplyRules, "drive_file_move", false),
            (Key::OfflineWindow, "cloud_off", false),
        ]
    );
    let runs: Vec<Run> = mailbox_actions(&state, inbox)
        .into_iter()
        .map(|e| e.run)
        .collect();
    assert_eq!(
        runs,
        [
            Run::NewSubfolder(inbox),
            Run::Command(Command::PinMailbox { id: inbox }),
            Run::Command(Command::HideMailbox { id: inbox }),
            Run::Command(Command::FocusMailbox {
                id: Some(inbox),
                account: None
            }),
            Run::MarkAllRead(inbox),
            Run::Command(Command::ApplyRules { mailbox: inbox }),
            Run::SyncWindow(inbox),
        ]
    );
    assert!(matches!(
        ends_in(&state, &runs[0], "Lists"),
        Command::CreateFolder { ref path, .. } if path.ends_with("Lists")
    ));
    assert_eq!(
        ends_in(&state, &runs[4], ""),
        Command::MarkMailboxRead { id: inbox }
    );
    assert_eq!(
        ends_in(&state, &runs[6], "200"),
        Command::SetSyncWindow {
            mailbox: inbox,
            window: Some(200)
        }
    );
    assert!(mailbox_actions(&state, app_core::IdGen::new(999).next_id()).is_empty());
}

#[test]
fn trash_and_junk_empty_and_a_folder_renames_and_deletes() {
    let mut state = synced_state();
    let account = *state.accounts.keys().next().unwrap();
    let trash = add_folder(&mut state, 77, "Trash", Some(MailboxRole::Trash));
    let junk = add_folder(&mut state, 78, "Junk", Some(MailboxRole::Junk));
    let plain = add_folder(&mut state, 79, "Projects", None);
    let tail = |state: &AppState, id| {
        let mut s = mailbox_shape(state, id);
        s.drain(..1);
        s
    };
    assert_eq!(
        tail(&state, trash),
        [
            (Key::Rename, "edit", false),
            (Key::PinFavourites, "push_pin", true),
            (Key::HideFolder, "visibility_off", false),
            (Key::FocusMailbox, "open_in_full", false),
            (Key::MarkAllRead, "mark_email_read", true),
            (Key::EmptyTrash, "delete", true),
            (Key::ApplyRules, "drive_file_move", false),
            (Key::OfflineWindow, "cloud_off", false),
            (Key::Delete, "delete", false),
        ]
    );
    let empty = |id| {
        mailbox_actions(&state, id)
            .into_iter()
            .find(|e| matches!(e.run, Run::Empty(..)))
            .map(|e| (e.label, e.run))
    };
    assert_eq!(
        empty(trash),
        Some((Key::EmptyTrash, Run::Empty(account, MailboxRole::Trash)))
    );
    assert_eq!(
        empty(junk),
        Some((Key::EmptyJunk, Run::Empty(account, MailboxRole::Junk)))
    );
    assert_eq!(empty(plain), None);
    assert_eq!(
        ends_in(&state, &Run::Empty(account, MailboxRole::Junk), ""),
        Command::EmptyJunk {
            account,
            confirm: true
        }
    );
    assert_eq!(
        ends_in(&state, &Run::Rename(plain), "Work"),
        Command::RenameFolder {
            id: plain,
            path: "Work".into()
        }
    );
    assert_eq!(
        ends_in(&state, &Run::Delete(plain), ""),
        Command::DeleteFolder {
            id: plain,
            confirm: true
        }
    );
    // Pinned and hidden: the toggles undo.
    let key = state.mailboxes[&plain].key();
    state.settings.pinned_mailboxes.push(key.clone());
    state.settings.hidden_mailboxes.insert(key);
    let labels: Vec<Key> = mailbox_actions(&state, plain)
        .iter()
        .map(|e| e.label)
        .collect();
    assert!(labels.contains(&Key::UnpinFavourites) && labels.contains(&Key::ShowFolder));
}

#[test]
fn an_account_tests_reenters_its_password_adds_an_address_and_removes() {
    let state = synced_state();
    let id: AccountId = *state.accounts.keys().next().unwrap();
    let list = account_actions(&state, id);
    assert_eq!(
        shape(&list),
        [
            (Key::Test, "verified_user", true),
            (Key::ReEnterPassword, "keyboard", true),
            (Key::SendingAddresses, "person", false),
            (Key::RemoveDots, "delete", false),
        ]
    );
    assert_eq!(list[0].run, Run::Command(Command::TestAccount { id }));
    assert!(matches!(list[1].run.flow(&state), Flow::Password(a) if a == id));
    assert_eq!(
        ends_in(&state, &list[2].run, "sales@example.org"),
        Command::SetIdentity {
            account: id,
            email: "sales@example.org".into(),
            name: None,
            signature: Some("sales@example.org".into()),
        }
    );
    assert_eq!(
        ends_in(&state, &list[3].run, ""),
        Command::RemoveAccount { id, confirm: true }
    );
    assert!(account_actions(&state, app_core::IdGen::new(999).next_id()).is_empty());
}

#[test]
fn every_icon_the_lists_return_is_in_icons() {
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let account = *state.accounts.keys().next().unwrap();
    let trash = add_folder(&mut state, 77, "Trash", Some(MailboxRole::Trash));
    let junk = add_folder(&mut state, 78, "Junk", Some(MailboxRole::Junk));
    let mut all = thread_actions(&state, &threads);
    set_flags(&mut state, threads[0], true, true, true);
    all.extend(thread_actions(&state, &[threads[0]]));
    for id in [inbox, trash, junk] {
        all.extend(mailbox_actions(&state, id));
    }
    state.view.mailbox = Some(junk);
    all.extend(thread_actions(&state, &[threads[0]]));
    all.extend(account_actions(&state, account));
    for entry in all {
        assert!(
            app_front::actions::ICONS.contains(&entry.icon),
            "{} missing from ICONS",
            entry.icon
        );
    }
}
