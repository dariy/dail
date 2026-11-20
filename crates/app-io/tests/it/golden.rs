//! The golden profiles (STORE-6): profiles written by released builds and frozen under
//! `crates/app-io/testdata/golden/<name>/` (inside app-io, so that the device tests find it; `cargo xtask android test` pushes a crate's `testdata`). Every later version must open each of them
//! with its accounts, settings, drafts, queued operations, approvals and blobs intact, and with
//! nothing quarantined. A golden profile is never edited or regenerated once a store build wrote
//! it; a format change adds a migration (or a serde default) instead.
//!
//! `DAIL_GOLDEN_FREEZE=<name> cargo test -p app-io --test it golden::freeze -- --ignored` writes a
//! new one with this build. A name ending in `-remote` writes a remote profile (ADR 0017:
//! `[remote]`, no accounts) instead, with [`remote_contents`]; it is checked against those.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use app_core::{
    AccountId, AppState, ApprovalId, ApprovalRequest, DraftId, LinkStatus, MailDelta, OpId, Origin,
    PersistDelta, SecretRef, ServerEndpoint, Settings, ViewPrefs,
};
use app_io::{ProfileDirs, ProfileStore, Store};

use super::store::{account, cache, draft, op, uuid};

/// The raw message every golden profile stores as a blob.
const BLOB: &[u8] = b"From: a@example.org\r\nSubject: Golden\r\n\r\nKept across versions.\r\n";

fn golden_root() -> PathBuf {
    // The run-time manifest dir first, for test runs on a device (AGENTS.md §6).
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    Path::new(&manifest).join("testdata/golden")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// What every golden profile holds; [`freeze`] writes it and [`every_golden_profile_opens_intact`]
/// checks it.
fn contents() -> Vec<PersistDelta> {
    let settings = Settings {
        poll_secs: 300,
        undo_send_ms: 15_000,
        ..Settings::default()
    };
    let mut mail = cache(1, 100);
    mail.drafts = vec![draft(800, 1)];
    mail.ops = vec![op(700, 2)];
    mail.approvals = vec![ApprovalRequest {
        id: ApprovalId::from_uuid(uuid(600)),
        draft: DraftId::from_uuid(uuid(800)),
        account: AccountId::from_uuid(uuid(1)),
        origin: Origin::Mcp,
        requested_ms: 3,
    }];
    vec![
        PersistDelta::Settings(settings),
        PersistDelta::Account(account(1)),
        PersistDelta::Mail(mail),
    ]
}

/// Whether the golden profile `name` is a remote profile.
fn is_remote(name: &str) -> bool {
    name.ends_with("-remote")
}

/// The pinned dail server of the remote golden profiles.
fn golden_server() -> ServerEndpoint {
    ServerEndpoint {
        host: "home.tail1234.ts.net".to_owned(),
        port: 7443,
        offered: None,
        pairing: None,
        fingerprint: Some("5e".repeat(32)),
        token: Some(SecretRef::for_server(uuid(900))),
    }
}

/// What every remote golden profile holds: settings, the view and the pinned server. Its mail is
/// the server's (a remote profile's cache holds no accounts of its own).
fn remote_contents() -> Vec<PersistDelta> {
    let settings = Settings {
        poll_secs: 300,
        undo_send_ms: 15_000,
        ..Settings::default()
    };
    vec![
        PersistDelta::Settings(settings),
        PersistDelta::View(ViewPrefs {
            sidebar_collapsed: true,
            ..ViewPrefs::default()
        }),
        PersistDelta::Remote(Some(golden_server())),
    ]
}

#[test]
#[ignore = "writes a golden profile; run by hand with DAIL_GOLDEN_FREEZE=<name>"]
fn freeze() {
    let name = std::env::var("DAIL_GOLDEN_FREEZE").unwrap();
    let dir = golden_root().join(&name);
    assert!(
        !dir.exists(),
        "a golden profile is never rewritten: {}",
        dir.display()
    );
    let dirs = ProfileDirs::at(&dir);
    dirs.ensure().unwrap();
    {
        let store = ProfileStore::open(&dirs).unwrap();
        let deltas = if is_remote(&name) {
            remote_contents()
        } else {
            contents()
        };
        for delta in deltas {
            store.apply(&delta).unwrap();
        }
        store.put_blob(BLOB).unwrap();
    }
    // Only what a version must read: no backups, logs or lock.
    for extra in [
        dirs.backups_dir(),
        dirs.logs_dir(),
        dirs.index_dir(),
        dirs.updates_dir(),
    ] {
        let _ = std::fs::remove_dir_all(extra);
    }
    let _ = std::fs::remove_file(dirs.lock_file());
}

#[test]
fn every_golden_profile_opens_intact() {
    let mut names: Vec<_> = std::fs::read_dir(golden_root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        // `settings/` holds the golden settings documents (ADR 0015), not a profile.
        .filter(|path| path.join("state.redb").exists())
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no golden profile");
    let expected: Vec<MailDelta> = contents()
        .into_iter()
        .filter_map(|delta| match delta {
            PersistDelta::Mail(mail) => Some(mail),
            _ => None,
        })
        .collect();
    let mail = &expected[0];
    for golden in names {
        let scratch = std::env::temp_dir().join(format!(
            "dail-golden-{}-{}",
            std::process::id(),
            golden.file_name().unwrap().to_string_lossy()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        copy_dir(&golden, &scratch);
        let store = ProfileStore::open(&ProfileDirs::at(&scratch)).unwrap();
        let state = store.load_snapshot().unwrap().unwrap();
        let at = golden.display();
        if is_remote(&golden.file_name().unwrap().to_string_lossy()) {
            check_remote(&store, &state, &at.to_string());
            drop(store);
            std::fs::remove_dir_all(scratch).unwrap();
            continue;
        }
        assert!(state.link.is_none(), "{at}");
        assert!(
            state.status.quarantined.is_empty(),
            "{at}: {:?}",
            state.status.quarantined
        );
        assert_eq!(state.settings.poll_secs, 300, "{at}");
        assert_eq!(state.settings.undo_send_ms, 15_000, "{at}");
        let account = &state.accounts[&AccountId::from_uuid(uuid(1))];
        assert_eq!(account.email, "me1@example.org", "{at}");
        assert_eq!(account.imap.host, "mail.example.org", "{at}");
        assert_eq!(state.drafts.len(), 1, "{at}");
        let kept = &state.drafts[&DraftId::from_uuid(uuid(800))];
        assert_eq!(
            (kept.subject.as_str(), kept.body_text.as_str()),
            ("Draft 800", "Hello"),
            "{at}"
        );
        assert_eq!(
            state.outbox.keys().copied().collect::<Vec<_>>(),
            [OpId::from_uuid(uuid(700))],
            "{at}"
        );
        assert_eq!(
            state.outbox[&OpId::from_uuid(uuid(700))],
            mail.ops[0],
            "{at}"
        );
        assert_eq!(state.approvals.len(), 1, "{at}");
        assert_eq!(state.messages.len(), mail.messages.len(), "{at}");
        assert_eq!(state.threads.len(), mail.threads.len(), "{at}");
        assert_eq!(state.mailboxes.len(), mail.mailboxes.len(), "{at}");
        assert_eq!(
            store.get_blob(&app_io::blob_id(BLOB)).unwrap().as_deref(),
            Some(BLOB),
            "{at}"
        );
        drop(store);
        std::fs::remove_dir_all(scratch).unwrap();
    }
}

/// A remote golden profile opened: the pinned server in mirror mode, its settings and view, no
/// accounts, nothing quarantined, the blob.
fn check_remote(store: &ProfileStore, state: &AppState, at: &str) {
    assert!(
        state.status.quarantined.is_empty(),
        "{at}: {:?}",
        state.status.quarantined
    );
    assert_eq!(state.settings.poll_secs, 300, "{at}");
    assert_eq!(state.settings.undo_send_ms, 15_000, "{at}");
    assert!(state.view.prefs().sidebar_collapsed, "{at}");
    assert!(state.accounts.is_empty(), "{at}");
    let link = state.link.as_ref().unwrap();
    assert_eq!(link.status, LinkStatus::Connecting, "{at}");
    assert_eq!(link.server.as_ref(), Some(&golden_server()), "{at}");
    assert_eq!(
        store.get_blob(&app_io::blob_id(BLOB)).unwrap().as_deref(),
        Some(BLOB),
        "{at}"
    );
}
