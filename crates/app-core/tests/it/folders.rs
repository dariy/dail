//! `collapse_folders` / `expand_folders` (UX-3) and `list_mailboxes`' hierarchy (UX-2).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::Mail;
use app_core::{
    AccountId, Command, CoreError, Effect, FolderKey, MailboxRole, PersistDelta, ProviderPreset,
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
