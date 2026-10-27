//! The folder tree projection (UX-2).

use app_core::{AccountId, AppState, FolderKey, Mailbox, MailboxId, MailboxRole};
use app_front::mailbox::{FolderNode, rows, tree};

fn state(delimiter: &str, folders: &[(&str, Option<MailboxRole>, u32)]) -> (AppState, AccountId) {
    let account: AccountId = app_core::IdGen::new(1).next_id();
    let mut state = AppState::default();
    for (n, (path, role, unread)) in folders.iter().enumerate() {
        let id: MailboxId = app_core::IdGen::new(100 + n as u128).next_id();
        state.mailboxes.insert(
            id,
            Mailbox {
                id,
                account,
                path: (*path).to_owned(),
                delimiter: Some(delimiter.to_owned()),
                role: *role,
                uid_validity: None,
                uid_next: None,
                unread: *unread,
                total: 0,
                highest_modseq: None,
                sync_window: None,
                history_complete: false,
                last_fetch_ms: None,
                custom_keywords: None,
                identity: None,
            },
        );
    }
    (state, account)
}

/// `label@depth` per row, `*` for nodes without a mailbox.
fn outline(nodes: &[FolderNode]) -> Vec<String> {
    rows(nodes, &|_| false)
        .iter()
        .map(|n| {
            let synthetic = if n.mailbox.is_none() { "*" } else { "" };
            format!("{}{synthetic}@{}", n.label, n.depth)
        })
        .collect()
}

#[test]
fn user_folders_nest_below_the_role_folders() {
    let (state, account) = state(
        "/",
        &[
            ("Work/Clients/Acme", None, 2),
            ("INBOX", Some(MailboxRole::Inbox), 1),
            ("Personal/Acme", None, 0),
            ("Sent", Some(MailboxRole::Sent), 0),
            ("work", None, 0),
            ("Work", None, 3),
        ],
    );
    assert_eq!(
        outline(&tree(&state, account)),
        [
            "Inbox@0",
            "Sent@0",
            "Personal*@0",
            "Acme@1",
            "Work@0",
            "Clients*@1",
            "Acme@2",
            "work@0",
        ]
    );
}

#[test]
fn missing_parents_are_synthetic_and_roll_up_unread() {
    let (state, account) = state(
        "/",
        &[("a/b/c", None, 2), ("a/b/d", None, 5), ("a/e", None, 1)],
    );
    let nodes = tree(&state, account);
    assert_eq!(nodes.len(), 1);
    let a = &nodes[0];
    assert_eq!(
        a.key,
        FolderKey {
            account,
            path: "a".into()
        }
    );
    assert_eq!((a.mailbox, a.unread, a.unread_below), (None, 0, 8));
    assert_eq!(a.children[0].unread_below, 7);
    assert!(a.is_parent() && !a.children[1].is_parent());
}

#[test]
fn inbox_children_move_to_the_top_level_with_dot_delimiters() {
    let (state, account) = state(
        ".",
        &[
            ("INBOX", Some(MailboxRole::Inbox), 0),
            ("INBOX.Archive", Some(MailboxRole::Archive), 0),
            ("INBOX.Archive.2023", None, 0),
            ("INBOX.Work", None, 4),
            ("INBOX.Work.Old", None, 0),
        ],
    );
    assert_eq!(
        outline(&tree(&state, account)),
        ["Inbox@0", "Archive@0", "2023@1", "Work@0", "Old@1"]
    );
}

#[test]
fn gmail_role_folders_stay_flat_and_empty_parents_vanish() {
    let (state, account) = state(
        "/",
        &[
            ("INBOX", Some(MailboxRole::Inbox), 0),
            ("[Gmail]/Sent Mail", Some(MailboxRole::Sent), 0),
            ("[Gmail]/All Mail", Some(MailboxRole::All), 0),
            ("[Gmail]/Trash", Some(MailboxRole::Trash), 0),
            ("label/nested", None, 0),
        ],
    );
    assert_eq!(
        outline(&tree(&state, account)),
        [
            "Inbox@0",
            "Sent@0",
            "All Mail@0",
            "Trash@0",
            "label*@0",
            "nested@1"
        ]
    );
}

#[test]
fn collapsed_nodes_hide_their_children() {
    let (state, account) = state("/", &[("a", None, 0), ("a/b", None, 0), ("c", None, 0)]);
    let nodes = tree(&state, account);
    let shown: Vec<_> = rows(&nodes, &|key| key.path == "a")
        .iter()
        .map(|n| n.label.clone())
        .collect();
    assert_eq!(shown, ["a", "c"]);
}

#[test]
fn focus_mode_shows_only_the_focused_subtree_or_account() {
    let (mut state, account) = state(
        "/",
        &[
            ("INBOX", Some(MailboxRole::Inbox), 0),
            ("Work", None, 1),
            ("Work/Acme", None, 2),
            ("Home", None, 0),
        ],
    );
    let work = state
        .mailboxes
        .values()
        .find(|m| m.path == "Work")
        .unwrap()
        .id;
    state.view.focused = Some(app_core::FocusTarget::Mailbox(work));
    let nodes = app_front::mailbox::sidebar_tree(&state, account);
    assert_eq!(outline(&nodes), ["Work@0", "Acme@1"]);
    assert!(app_front::mailbox::shows_account(&state, account));
    let other: AccountId = app_core::IdGen::new(77).next_id();
    assert!(!app_front::mailbox::shows_account(&state, other));
    assert!(app_front::mailbox::sidebar_tree(&state, other).is_empty());
    state.view.focused = Some(app_core::FocusTarget::Account(account));
    assert_eq!(app_front::mailbox::sidebar_tree(&state, account).len(), 3);
}

#[test]
fn hidden_folders_and_their_subfolders_leave_the_rows_unless_shown() {
    let (mut state, account) = state(
        "/",
        &[
            ("INBOX", Some(MailboxRole::Inbox), 0),
            ("Work", None, 0),
            ("Work/Acme", None, 0),
            ("Home", None, 0),
        ],
    );
    state.settings.hidden_mailboxes.insert(FolderKey {
        account,
        path: "Work".into(),
    });
    let nodes = tree(&state, account);
    let labels = |rows: Vec<&FolderNode>| rows.iter().map(|n| n.label.clone()).collect::<Vec<_>>();
    assert_eq!(
        labels(app_front::mailbox::visible_rows(&state, account, &nodes)),
        ["Inbox", "Home"]
    );
    assert_eq!(
        labels(app_front::mailbox::visible_rows_with(
            &state, account, &nodes, true
        ))
        .len(),
        4
    );
    assert_eq!(app_front::mailbox::hidden_count(&state, account), 1);
    state.settings.pinned_mailboxes.push(FolderKey {
        account,
        path: "Home".into(),
    });
    let favourites: Vec<_> = app_front::mailbox::favourites(&state)
        .iter()
        .map(|m| m.path.clone())
        .collect();
    assert_eq!(favourites, ["Home"]);
}
