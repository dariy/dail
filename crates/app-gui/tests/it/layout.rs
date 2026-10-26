//! The three-pane layout: sidebar, thread list and reader emit the right commands.

use crate::support::{apply, gui, inbox_of, synced_state};
use app_core::{BodyState, Command, MailboxRole, Pane, SettingKey};
use app_gui::{Background, Purpose};
use egui::accesskit::Role;
use egui_kittest::kittest::{By, Queryable};

/// An action button: its label followed by its key hint (`Archive (E)`), unlike the mailbox
/// row of the same name, a button that only starts with the same letters (`Starred`, the list
/// filter) or a list row's hover action (`Star conversation`).
fn action(label: &str) -> By<'_> {
    By::new().role(Role::Button).predicate(move |node| {
        node.label().is_some_and(|l| {
            l.find(label)
                .is_some_and(|at| l[at + label.len()..].starts_with(|c: char| !c.is_alphanumeric()))
                && l != label
                && !l.contains(", ")
                && !l.ends_with(" conversation")
        })
    })
}

#[test]
fn the_sidebar_lists_mailboxes_and_opens_them() {
    let state = synced_state();
    let sent = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Sent))
        .unwrap()
        .id;
    let mut h = gui(state);
    h.run();
    // A mailbox row is a button named after the mailbox (its parts are its children).
    for name in ["Inbox", "Sent", "Drafts", "Archive"] {
        h.get(
            By::new()
                .role(Role::Button)
                .label_contains(name)
                .label_contains(name),
        );
    }
    h.get(By::new().role(Role::Button).label("Sent")).click();
    h.run();
    assert_eq!(h.state_mut().take(), [Command::OpenMailbox { id: sent }]);
}

#[test]
fn the_sidebar_shows_a_running_backfill() {
    let mut state = synced_state();
    let (inbox, _) = inbox_of(&state);
    state.mailboxes.get_mut(&inbox).unwrap().total = 8;
    state.status.backfilling.insert(inbox);
    let mut h = gui(state);
    h.run();
    // 2 of 8 cached.
    h.get_by_label("↓25%");
}

#[test]
fn the_list_shows_the_open_mailbox_and_opens_threads() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut h = gui(state);
    h.run();
    assert!(h.query_by_label("Lunch?").is_none(), "no mailbox open yet");
    apply(&mut h, Command::OpenMailbox { id: inbox });
    h.get_by_label("Q3 numbers");
    h.get_by_label("1 unread");
    // Newest first: Carol's "Lunch?" (12:02) above Bob's "Q3 numbers" (09:41).
    let lunch = h.get_by_label("Lunch?").rect().min.y;
    let q3 = h.get_by_label("Q3 numbers").rect().min.y;
    assert!(lunch < q3);
    h.get_by_label("Lunch?").click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::OpenThread { id: threads[0] }]
    );
}

#[test]
fn the_reader_shows_the_thread_and_its_actions_act_on_it() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[1] });
    h.get_by_label_contains("Bob <bob@example.org>");
    h.state_mut().out.clear();
    let ids = vec![threads[1]];
    for (label, expected) in [
        (
            "Archive",
            Command::ArchiveThreads {
                ids: ids.clone(),
                messages: Vec::new(),
            },
        ),
        (
            "Trash",
            Command::TrashThreads {
                ids: ids.clone(),
                messages: Vec::new(),
            },
        ),
        (
            "Star",
            Command::Star {
                ids: ids.clone(),
                messages: Vec::new(),
            },
        ),
        (
            "Mark unread",
            Command::MarkUnread {
                ids: ids.clone(),
                messages: Vec::new(),
            },
        ),
        ("Close", Command::CloseThread {}),
    ] {
        h.get(action(label)).click();
        h.run();
        // Archive and Trash are followed by opening a neighbour (UX-12).
        assert_eq!(h.state_mut().take().first(), Some(&expected), "{label}");
    }
    h.get(By::new().role(Role::Button).label_contains("Reply all"))
        .click();
    h.run();
    assert!(matches!(
        h.state_mut().take()[..],
        [Command::ReplyAll { .. }]
    ));
}

#[test]
fn the_reader_asks_for_bodies_once_per_body_state() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[1] });
    let background: Vec<_> = h
        .state()
        .out
        .iter()
        .filter(|r| r.purpose == Purpose::Background(Background::ReaderBodies(threads[1])))
        .map(|r| r.cmd.clone())
        .collect();
    assert_eq!(
        background,
        [Command::GetThread {
            id: threads[1],
            include_body: true,
            rich_body: true,
        }],
        "one request, however many frames ran"
    );
    h.state_mut().pump();
    h.run();
    assert!(h.state().out.is_empty(), "answered: nothing asked again");
    // The memory engine never downloads, so the body state stays and the reader keeps waiting.
    let message = h.state().engine.state().threads[&threads[1]].messages[0];
    assert!(matches!(
        h.state().engine.state().messages[&message].body,
        BodyState::NotFetched
    ));
    h.get_by_label("Loading…");
}

#[test]
fn creating_a_draft_opens_it_in_the_composer() {
    let state = synced_state();
    let mut h = gui(state);
    h.run();
    h.get_by_label_contains("Compose").click();
    h.run();
    let sent = h.state_mut().pump();
    assert!(matches!(sent[..], [Command::CreateDraft { .. }]));
    h.run();
    let follow = h.state_mut().pump();
    assert!(
        matches!(follow[..], [Command::OpenComposer { .. }]),
        "{follow:?}"
    );
    h.run();
    assert_eq!(h.state().engine.state().view.focus, Pane::Composer);
}

#[test]
fn the_sidebar_collapses_and_the_theme_follows_the_setting() {
    let mut h = gui(synced_state());
    h.run();
    h.get_by_label("Collapse sidebar (Ctrl+B)").click();
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::ToggleSidebar {}]);
    h.run();
    assert!(
        h.query_by_label("+ Add account").is_none(),
        "collapsed to the rail"
    );
    h.get_by_label("Expand sidebar (Ctrl+B)").click();
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::ToggleSidebar {}]);
    h.run();
    h.get_by_label("Inbox");

    for (value, dark) in [(1_u64, false), (2, true)] {
        apply(
            &mut h,
            Command::SetSetting {
                key: SettingKey::Theme,
                value: value.into(),
            },
        );
        assert_eq!(h.ctx.global_style().visuals.dark_mode, dark);
    }
}

#[test]
fn ctrl_and_shift_clicks_check_threads_for_bulk_actions() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    h.state_mut().out.clear();
    h.get_by_label("Lunch?")
        .click_modifiers(egui::Modifiers::COMMAND);
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::CheckThreads {
            ids: vec![threads[0]]
        }]
    );
    // Shift: everything from the cursor (none yet: the first row) to the clicked row.
    h.get_by_label("Q3 numbers")
        .click_modifiers(egui::Modifiers::SHIFT);
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::CheckThreads {
            ids: threads.clone()
        }]
    );
    apply(
        &mut h,
        Command::CheckThreads {
            ids: threads.clone(),
        },
    );
    h.get_by_label("2 checked");
    // The bulk bar's button (the mailbox row of that name is a toggle).
    h.get(
        By::new()
            .role(Role::Button)
            .label("Archive")
            .predicate(|n| n.toggled().is_none()),
    )
    .click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::ArchiveThreads {
            ids: threads.clone(),
            messages: Vec::new()
        }]
    );
    h.get(By::new().role(Role::Button).label_contains("Uncheck all"))
        .click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::UncheckThreads { ids: vec![] }]
    );
}

#[test]
fn the_list_shows_all_inboxes_once_opened() {
    let state = synced_state();
    let mut h = gui(state);
    apply(&mut h, Command::OpenUnifiedInbox {});
    h.get_by_label("All inboxes");
    h.get_by_label("Lunch?");
    h.get_by_label("Q3 numbers");
}

#[test]
fn the_cursor_row_offers_its_actions_by_name() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(
        &mut h,
        Command::SelectThreads {
            ids: vec![threads[0]],
        },
    );
    apply(
        &mut h,
        Command::FocusPane {
            pane: app_core::Pane::List,
        },
    );
    h.state_mut().out.clear();
    h.get(By::new().role(Role::Button).label("Archive conversation"))
        .click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::ArchiveThreads {
            ids: vec![threads[0]],
            messages: Vec::new()
        }]
    );
    h.get(By::new().role(Role::Button).label("Trash conversation"));
}

#[test]
fn a_row_dragged_onto_a_folder_moves_there() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let archive = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Archive))
        .unwrap()
        .id;
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    h.state_mut().out.clear();
    let from = h.get_by_label("Lunch?").rect().center();
    let to = h
        .get(By::new().role(Role::Button).label("Archive"))
        .rect()
        .center();
    let pointer = |h: &mut egui_kittest::Harness<'_, _>, event: egui::Event| {
        h.input_mut().events.push(event);
        h.step();
    };
    pointer(&mut h, egui::Event::PointerMoved(from));
    pointer(
        &mut h,
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    );
    for step in 1..=8 {
        let at = from + (to - from) * (step as f32 / 8.0);
        pointer(&mut h, egui::Event::PointerMoved(at));
    }
    pointer(
        &mut h,
        egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    );
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::MoveThreads {
            ids: vec![threads[0]],
            messages: Vec::new(),
            to: archive,
        }]
    );
}

#[test]
fn the_list_groups_rows_by_date() {
    let state = synced_state();
    let (inbox, _) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    let groups = [
        "Later",
        "Today",
        "Yesterday",
        "This week",
        "Earlier this month",
    ];
    let found = h
        .query_all(By::new().role(Role::Heading).predicate(move |n| {
            n.label()
                .is_some_and(|l| groups.contains(&l.as_str()) || l.contains(" 20"))
        }))
        .count();
    assert!(found >= 1, "no date group header");
}
