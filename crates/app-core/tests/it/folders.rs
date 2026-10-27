//! `collapse_folders` / `expand_folders` (UX-3) and `list_mailboxes`' hierarchy (UX-2).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::Mail;
use app_core::{
    AccountId, Command, CoreError, Effect, FocusTarget, FolderKey, MailboxRole, PersistDelta,
    ProviderPreset,
};
use serde_json::json;

fn nested() -> Mail {
    Mail::new(
        ProviderPreset::Custom,
        &[
            ("INBOX", None),
            ("Sent", Some(MailboxRole::Sent)),
            ("Work", None),
            ("Work/Clients", None),
            ("Work/Clients/Acme", None),
            ("Lists/rust", None),
        ],
    )
}

fn key(m: &Mail, path: &str) -> FolderKey {
    FolderKey {
        account: m.account,
        path: path.to_owned(),
    }
}

fn collapsed(m: &Mail) -> Vec<String> {
    m.engine
        .state()
        .view
        .collapsed
        .iter()
        .map(|k| k.path.clone())
        .collect()
}

#[test]
fn collapse_and_expand_are_idempotent_and_saved() {
    let mut m = nested();
    let undo = m.engine.history().undo_len();
    let outcome = m
        .run(Command::CollapseFolders {
            keys: vec![key(&m, "Work")],
        })
        .unwrap();
    assert_eq!(
        outcome.receipt.data,
        json!({ "changed": 1, "collapsed": 1 })
    );
    assert!(outcome.effects.iter().any(|e| matches!(
        e,
        Effect::PersistState { delta: PersistDelta::View(prefs) } if prefs.collapsed.len() == 1
    )));
    let again = m
        .run(Command::CollapseFolders {
            keys: vec![key(&m, "Work")],
        })
        .unwrap();
    assert_eq!(again.receipt.data, json!({ "changed": 0, "collapsed": 1 }));
    assert!(again.effects.is_empty(), "nothing changed, nothing saved");
    m.run(Command::ExpandFolders {
        keys: vec![key(&m, "Work")],
    })
    .unwrap();
    assert!(collapsed(&m).is_empty());
    assert_eq!(
        m.engine.history().undo_len(),
        undo,
        "view commands are not undoable"
    );
}

#[test]
fn empty_keys_collapse_every_parent_and_account_and_expand_all() {
    let mut m = nested();
    m.run(Command::CollapseFolders { keys: Vec::new() })
        .unwrap();
    assert_eq!(collapsed(&m), ["", "Lists", "Work", "Work/Clients"]);
    m.run(Command::ExpandFolders { keys: Vec::new() }).unwrap();
    assert!(collapsed(&m).is_empty());
}

#[test]
fn unknown_accounts_and_folders_are_rejected() {
    let mut m = nested();
    let stranger = AccountId::from_uuid(uuid::Uuid::from_u128(9));
    let err = m
        .run(Command::CollapseFolders {
            keys: vec![FolderKey {
                account: stranger,
                path: String::new(),
            }],
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }), "{err:?}");
    // A path prefix that is not a whole segment is not a folder.
    for path in ["Wor", "Nope", "Work/Cl"] {
        let err = m
            .run(Command::CollapseFolders {
                keys: vec![key(&m, path)],
            })
            .unwrap_err();
        assert!(
            matches!(err, CoreError::InvalidArgument { .. }),
            "{path}: {err:?}"
        );
    }
    // `Lists` is not cached (only `Lists/rust` is) but is a folder of the tree.
    m.run(Command::CollapseFolders {
        keys: vec![key(&m, "Lists"), key(&m, "")],
    })
    .unwrap();
    assert_eq!(collapsed(&m), ["", "Lists"]);
}

#[test]
fn list_mailboxes_reports_parent_and_depth() {
    let mut m = nested();
    let outcome = m.run(Command::ListMailboxes { account: None }).unwrap();
    let acme = outcome.receipt.data["mailboxes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == "Work/Clients/Acme")
        .unwrap()
        .clone();
    assert_eq!(acme["parent"], "Work/Clients");
    assert_eq!(acme["depth"], 2);
}

#[test]
fn focus_mailbox_opens_it_and_navigation_outside_ends_it() {
    let mut m = nested();
    let work = m.mailbox("Work");
    let acme = m.mailbox("Work/Clients/Acme");
    let inbox = m.mailbox("INBOX");
    let outcome = m
        .run(Command::FocusMailbox {
            id: Some(work),
            account: None,
        })
        .unwrap();
    assert_eq!(outcome.receipt.data["focused"], json!({ "mailbox": work }));
    assert!(outcome.effects.iter().any(|e| matches!(
        e,
        Effect::PersistState { delta: PersistDelta::View(prefs) }
            if prefs.focused == Some(FocusTarget::Mailbox(work))
    )));
    let view = &m.engine.state().view;
    assert_eq!(view.mailbox, Some(work));
    // A folder below stays in focus; another one ends it.
    m.run(Command::OpenMailbox { id: acme }).unwrap();
    assert_eq!(
        m.engine.state().view.focused,
        Some(FocusTarget::Mailbox(work))
    );
    m.run(Command::OpenMailbox { id: inbox }).unwrap();
    assert_eq!(m.engine.state().view.focused, None);
}

#[test]
fn focus_on_an_account_opens_its_inbox_and_clear_focus_ends_it() {
    let mut m = nested();
    let account = m.account;
    let outcome = m
        .run(Command::FocusMailbox {
            id: None,
            account: Some(account),
        })
        .unwrap();
    assert_eq!(
        outcome.receipt.data["focused"],
        json!({ "account": account })
    );
    assert_eq!(m.engine.state().view.mailbox, Some(m.mailbox("INBOX")));
    m.run(Command::OpenMailbox {
        id: m.mailbox("Sent"),
    })
    .unwrap();
    assert!(m.engine.state().view.focused.is_some(), "same account");
    let outcome = m.run(Command::ClearFocus {}).unwrap();
    assert_eq!(outcome.receipt.data, json!({ "cleared": true }));
    let outcome = m.run(Command::ClearFocus {}).unwrap();
    assert_eq!(outcome.receipt.data, json!({ "cleared": false }));
}

#[test]
fn focus_needs_exactly_one_existing_target_and_ends_with_it() {
    let mut m = nested();
    for (id, account) in [(None, None), (Some(m.mailbox("Work")), Some(m.account))] {
        let err = m.run(Command::FocusMailbox { id, account }).unwrap_err();
        assert!(matches!(err, CoreError::InvalidArgument { .. }), "{err:?}");
    }
    let err = m
        .run(Command::FocusMailbox {
            id: None,
            account: Some(AccountId::from_uuid(uuid::Uuid::from_u128(9))),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }), "{err:?}");
    m.run(Command::FocusMailbox {
        id: Some(m.mailbox("Work")),
        account: None,
    })
    .unwrap();
    m.run(Command::OpenUnifiedInbox {}).unwrap();
    assert_eq!(
        m.engine.state().view.focused,
        None,
        "the unified inbox leaves focus"
    );
    m.run(Command::FocusMailbox {
        id: Some(m.mailbox("Lists/rust")),
        account: None,
    })
    .unwrap();
    // The server no longer lists it.
    m.feed(app_core::Event::MailboxesListed {
        account: m.account,
        mailboxes: vec![app_core::RemoteMailbox {
            path: "INBOX".into(),
            delimiter: Some("/".into()),
            special_use: None,
            selectable: true,
        }],
    });
    assert_eq!(m.engine.state().view.focused, None);
}

#[test]
fn pin_and_hide_are_undoable_settings_kept_by_path() {
    let mut m = nested();
    let work = m.mailbox("Work");
    let sent = m.mailbox("Sent");
    let outcome = m.run(Command::PinMailbox { id: work }).unwrap();
    assert_eq!(outcome.receipt.data["pinned"], true);
    assert!(outcome.effects.iter().any(|e| matches!(
        e,
        Effect::PersistState {
            delta: PersistDelta::Settings(_)
        }
    )));
    m.run(Command::PinMailbox { id: sent }).unwrap();
    let again = m.run(Command::PinMailbox { id: work }).unwrap();
    assert_eq!(again.receipt.data["changed"], false);
    let pinned: Vec<&str> = m
        .engine
        .state()
        .settings
        .pinned_mailboxes
        .iter()
        .map(|k| k.path.as_str())
        .collect();
    assert_eq!(pinned, ["Work", "Sent"], "in pinned order");
    m.run(Command::Undo {}).unwrap();
    assert_eq!(m.engine.state().settings.pinned_mailboxes.len(), 1);

    m.run(Command::HideMailbox { id: work }).unwrap();
    assert!(
        m.engine
            .state()
            .settings
            .hidden_mailboxes
            .contains(&key(&m, "Work"))
    );
    m.run(Command::ShowMailbox { id: work }).unwrap();
    assert!(m.engine.state().settings.hidden_mailboxes.is_empty());
    m.run(Command::Undo {}).unwrap();
    assert_eq!(
        m.engine.state().settings.hidden_mailboxes.len(),
        1,
        "hidden again"
    );
    m.run(Command::UnpinMailbox { id: work }).unwrap();
    assert!(m.engine.state().settings.pinned_mailboxes.is_empty());

    let gone = app_core::IdGen::new(99).next_id();
    let err = m.run(Command::HideMailbox { id: gone }).unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }));
}

#[test]
fn folder_management_validates_and_changes_the_cache_first() {
    let mut m = nested();
    let account = m.account;
    for bad in ["", "  ", "a//b", "x*", "x%", "/lead"] {
        let err = m
            .run(Command::CreateFolder {
                account,
                path: bad.into(),
            })
            .unwrap_err();
        assert!(matches!(err, CoreError::InvalidArgument { .. }), "{bad:?}");
    }
    let err = m
        .run(Command::CreateFolder {
            account,
            path: "work".into(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("exists"), "{err}");
    let created = m
        .run(Command::CreateFolder {
            account,
            path: "Work/New".into(),
        })
        .unwrap();
    assert!(created.receipt.undo_label.is_none());
    assert!(created.effects.iter().any(|e| matches!(
        e,
        Effect::ManageFolder { action: app_core::FolderAction::Create { path }, .. } if path == "Work/New"
    )));
    let work = m.mailbox("Work");
    m.run(Command::PinMailbox {
        id: m.mailbox("Work/Clients"),
    })
    .unwrap();
    m.run(Command::CollapseFolders {
        keys: vec![key(&m, "Work/Clients")],
    })
    .unwrap();
    let renamed = m
        .run(Command::RenameFolder {
            id: work,
            path: "Jobs".into(),
        })
        .unwrap();
    assert_eq!(renamed.receipt.undo_label.as_deref(), Some("Rename Work"));
    let paths: Vec<String> = m
        .engine
        .state()
        .mailboxes
        .values()
        .map(|mb| mb.path.clone())
        .collect();
    assert!(paths.contains(&"Jobs/Clients/Acme".to_owned()), "{paths:?}");
    assert_eq!(
        m.engine.state().settings.pinned_mailboxes[0].path,
        "Jobs/Clients"
    );
    assert!(
        m.engine
            .state()
            .view
            .collapsed
            .contains(&key(&m, "Jobs/Clients"))
    );
    for (path, why) in [("Jobs/Clients/X", "into itself"), ("Sent", "exists")] {
        let err = m
            .run(Command::RenameFolder {
                id: work,
                path: path.into(),
            })
            .unwrap_err();
        assert!(err.to_string().contains(why), "{err}");
    }
    let inbox = m.mailbox("INBOX");
    let err = m
        .run(Command::RenameFolder {
            id: inbox,
            path: "Box".into(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("inbox"), "{err}");
    m.run(Command::Undo {}).unwrap();
    assert_eq!(m.engine.state().mailboxes[&work].path, "Work");

    let err = m
        .run(Command::DeleteFolder {
            id: work,
            confirm: false,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::ConfirmationRequired { .. }));
    let err = m
        .run(Command::DeleteFolder {
            id: work,
            confirm: true,
        })
        .unwrap_err();
    assert!(err.to_string().contains("subfolders"), "{err}");
    let acme = m.mailbox("Work/Clients/Acme");
    let deleted = m
        .run(Command::DeleteFolder {
            id: acme,
            confirm: true,
        })
        .unwrap();
    assert_eq!(deleted.receipt.data["undoable"], false);
    assert!(!m.engine.state().mailboxes.contains_key(&acme));
}
