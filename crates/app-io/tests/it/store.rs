//! `RedbStore`, `BlobDir` and `ProfileStore`: round trips, reopen, migrations, routing.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use app_core::{
    Account, AccountId, AccountStatus, AppState, ApprovalId, ApprovalRequest, BodyState, Draft,
    DraftId, Endpoint, Envelope, FlagKind, Flags, MailDelta, Mailbox, MailboxId, MailboxRole,
    MessageId, OpId, OpState, Origin, PendingOp, PersistDelta, ProviderPreset, RemoteOp, SecretRef,
    Security, Thread, ThreadId,
};
use app_io::store::migrations;
use app_io::{BlobDir, IoError, ProfileDirs, ProfileStore, RedbStore, Store, blob_id};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-store-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn uuid(n: u128) -> uuid::Uuid {
    uuid::Uuid::from_u128(n)
}

fn account(n: u128) -> Account {
    let id = AccountId::from_uuid(uuid(n));
    let endpoint = Endpoint {
        host: "mail.example.org".into(),
        port: 993,
        security: Security::Tls,
    };
    Account {
        id,
        display_name: "Me".into(),
        email: format!("me{n}@example.org"),
        preset: ProviderPreset::Custom,
        imap: endpoint.clone(),
        smtp: endpoint,
        secret: SecretRef::for_account(id),
        status: AccountStatus::Unverified,
        identities: Vec::new(),
    }
}

fn cache(account: u128, n: u128) -> MailDelta {
    let account = AccountId::from_uuid(uuid(account));
    let mailbox = MailboxId::from_uuid(uuid(n));
    let thread = ThreadId::from_uuid(uuid(n + 1));
    let message = MessageId::from_uuid(uuid(n + 2));
    MailDelta {
        mailboxes: vec![Mailbox {
            id: mailbox,
            account,
            path: "INBOX".into(),
            delimiter: Some("/".into()),
            role: Some(MailboxRole::Inbox),
            uid_validity: Some(7),
            uid_next: Some(2),
            unread: 1,
            total: 1,
            highest_modseq: None,
            sync_window: None,
            history_complete: false,
            identity: None,
        }],
        messages: vec![Envelope {
            id: message,
            account,
            mailbox,
            uid: 1,
            message_id_hdr: Some("a@x".into()),
            in_reply_to: None,
            references: Vec::new(),
            thread,
            from: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            subject: "Hello".into(),
            date_ms: 5,
            flags: Flags::default(),
            size: 10,
            has_attachments: false,
            body: BodyState::NotFetched,
            moved_to: None,
        }],
        threads: vec![Thread {
            id: thread,
            account,
            messages: vec![message],
            subject: "Hello".into(),
            last_date_ms: 5,
        }],
        ..MailDelta::default()
    }
}

#[test]
fn redb_store_round_trips_and_survives_reopen() {
    let dir = scratch("redb");
    let path = dir.join("state.redb");
    let delta = cache(1, 100);
    {
        let store = RedbStore::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), migrations::CURRENT);
        store.apply_mail(&delta).unwrap();
    }
    let store = RedbStore::open(&path).unwrap();
    let mut state = AppState::default();
    store.load_into(&mut state).unwrap();
    let mut expected = AppState::default();
    delta.apply_to(&mut expected);
    assert_eq!(state, expected);

    // Removals are applied before upserts, in one transaction.
    store
        .apply_mail(&MailDelta {
            removed_messages: vec![delta.messages[0].id],
            removed_threads: vec![delta.threads[0].id],
            ..MailDelta::default()
        })
        .unwrap();
    let mut state = AppState::default();
    store.load_into(&mut state).unwrap();
    assert!(state.messages.is_empty() && state.threads.is_empty());
    assert_eq!(state.mailboxes.len(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

fn op(n: u128, created_ms: i64) -> PendingOp {
    PendingOp {
        id: OpId::from_uuid(uuid(n)),
        account: AccountId::from_uuid(uuid(1)),
        op: RemoteOp::SetFlag {
            flag: FlagKind::Seen,
            value: true,
            messages: vec![MessageId::from_uuid(uuid(102))],
        },
        state: OpState::Queued,
        attempts: 0,
        last_error: None,
        created_ms,
        next_attempt_ms: None,
        awaiting_sync: false,
    }
}

#[test]
fn redb_store_keeps_the_outbox_in_queue_order() {
    let dir = scratch("outbox");
    let path = dir.join("state.redb");
    {
        let store = RedbStore::open(&path).unwrap();
        // Keys are not in creation order; loading restores the queue order.
        store
            .apply_mail(&MailDelta {
                ops: vec![op(900, 2), op(800, 1), op(700, 3)],
                ..MailDelta::default()
            })
            .unwrap();
        store
            .apply_mail(&MailDelta {
                removed_ops: vec![OpId::from_uuid(uuid(700))],
                ..MailDelta::default()
            })
            .unwrap();
    }
    let store = RedbStore::open(&path).unwrap();
    let mut state = AppState::default();
    store.load_into(&mut state).unwrap();
    let order: Vec<OpId> = state.outbox.keys().copied().collect();
    assert_eq!(
        order,
        [OpId::from_uuid(uuid(800)), OpId::from_uuid(uuid(900))]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

fn draft(n: u128, created_ms: i64) -> Draft {
    Draft {
        id: DraftId::from_uuid(uuid(n)),
        account: AccountId::from_uuid(uuid(1)),
        reply_to: None,
        message_id_hdr: format!("{n}@example.org"),
        in_reply_to: None,
        references: Vec::new(),
        to: Vec::new(),
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: format!("Draft {n}"),
        body_text: "Hello".into(),
        quote: None,
        attachments: Vec::new(),
        created_ms,
        updated_ms: created_ms,
        saved_ms: None,
        save_error: None,
        send_at: None,
        sending: false,
        send_error: None,
        from: None,
    }
}

#[test]
fn redb_store_keeps_drafts_in_creation_order() {
    let dir = scratch("drafts");
    let path = dir.join("state.redb");
    {
        let store = RedbStore::open(&path).unwrap();
        store
            .apply_mail(&MailDelta {
                drafts: vec![draft(900, 2), draft(800, 1), draft(700, 3)],
                ..MailDelta::default()
            })
            .unwrap();
        store
            .apply_mail(&MailDelta {
                removed_drafts: vec![DraftId::from_uuid(uuid(700))],
                drafts: vec![Draft {
                    subject: "Edited".into(),
                    ..draft(900, 2)
                }],
                ..MailDelta::default()
            })
            .unwrap();
    }
    let store = RedbStore::open(&path).unwrap();
    let mut state = AppState::default();
    store.load_into(&mut state).unwrap();
    let drafts: Vec<(DraftId, String)> = state
        .drafts
        .values()
        .map(|d| (d.id, d.subject.clone()))
        .collect();
    assert_eq!(
        drafts,
        [
            (DraftId::from_uuid(uuid(800)), "Draft 800".to_owned()),
            (DraftId::from_uuid(uuid(900)), "Edited".to_owned()),
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn newer_schema_is_refused() {
    let dir = scratch("schema");
    let path = dir.join("state.redb");
    {
        let db = redb::Database::create(&path).unwrap();
        let txn = db.begin_write().unwrap();
        txn.open_table(redb::TableDefinition::<&str, u64>::new("meta"))
            .unwrap()
            .insert("schema_version", migrations::CURRENT + 1)
            .unwrap();
        txn.commit().unwrap();
    }
    let err = RedbStore::open(&path).unwrap_err();
    assert!(matches!(err, IoError::SchemaTooNew { found, .. } if found == migrations::CURRENT + 1));
    assert!(migrations::pending(migrations::CURRENT).is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn blobs_are_content_addressed() {
    let dir = scratch("blobs");
    let blobs = BlobDir::new(dir.join("blobs"));
    let id = blobs.put(b"Subject: hi\r\n\r\nbody").unwrap();
    assert_eq!(id, blob_id(b"Subject: hi\r\n\r\nbody"));
    assert_eq!(id.len(), 64);
    assert_eq!(blobs.put(b"Subject: hi\r\n\r\nbody").unwrap(), id);
    let path = blobs.path(&id).unwrap();
    assert!(path.starts_with(dir.join("blobs").join(&id[..2])));
    assert!(path.to_string_lossy().ends_with(".eml"));
    assert_eq!(
        blobs.get(&id).unwrap().as_deref(),
        Some(&b"Subject: hi\r\n\r\nbody"[..])
    );
    assert_eq!(blobs.get(&blob_id(b"other")).unwrap(), None);
    for bad in ["../../etc/passwd", "ABC", &id.to_uppercase()] {
        assert!(matches!(blobs.get(bad), Err(IoError::InvalidBlobId(_))));
    }
    assert_eq!(
        blob_id(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn profile_store_routes_deltas_and_merges_on_load() {
    let dir = scratch("profile");
    let dirs = ProfileDirs::at(&dir);
    {
        let store = ProfileStore::open(&dirs).unwrap();
        assert_eq!(store.load_snapshot().unwrap(), None);
        store.apply(&PersistDelta::Account(account(1))).unwrap();
        store.apply(&PersistDelta::Mail(cache(1, 100))).unwrap();
        // Cache of an account that is not in the config: dropped on load.
        store.apply(&PersistDelta::Mail(cache(2, 200))).unwrap();
        let blob = store.put_blob(b"raw").unwrap();
        assert_eq!(store.get_blob(&blob).unwrap().as_deref(), Some(&b"raw"[..]));
    }
    let config = std::fs::read_to_string(dirs.config_file()).unwrap();
    assert!(
        !config.contains("INBOX"),
        "mail cache stays out of config.toml"
    );
    let store = ProfileStore::open(&dirs).unwrap();
    let state = store.load_snapshot().unwrap().unwrap();
    assert_eq!(state.accounts.len(), 1);
    assert_eq!(state.mailboxes.len(), 1);
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.threads.len(), 1);
    assert!(
        state
            .messages
            .values()
            .all(|m| m.account == AccountId::from_uuid(uuid(1)))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn redb_store_keeps_approval_requests_in_request_order() {
    let dir = scratch("approvals");
    let path = dir.join("state.redb");
    let request = |n: u128, requested_ms: i64| ApprovalRequest {
        id: ApprovalId::from_uuid(uuid(n)),
        draft: DraftId::from_uuid(uuid(n + 1)),
        account: AccountId::from_uuid(uuid(1)),
        origin: Origin::Mcp,
        requested_ms,
    };
    {
        let store = RedbStore::open(&path).unwrap();
        store
            .apply_mail(&MailDelta {
                approvals: vec![request(900, 2), request(800, 1), request(700, 3)],
                ..MailDelta::default()
            })
            .unwrap();
        store
            .apply_mail(&MailDelta {
                removed_approvals: vec![ApprovalId::from_uuid(uuid(700))],
                ..MailDelta::default()
            })
            .unwrap();
    }
    let store = RedbStore::open(&path).unwrap();
    let mut state = AppState::default();
    store.load_into(&mut state).unwrap();
    let ids: Vec<ApprovalId> = state.approvals.keys().copied().collect();
    assert_eq!(
        ids,
        [
            ApprovalId::from_uuid(uuid(800)),
            ApprovalId::from_uuid(uuid(900))
        ]
    );
    assert_eq!(state.approvals[&ids[0]], request(800, 1));
}

#[test]
fn compaction_deletes_old_unused_blobs_only() {
    let dir = scratch("compact");
    let dirs = ProfileDirs::at(&dir);
    dirs.ensure().unwrap();
    let store = ProfileStore::open(&dirs).unwrap();
    store.apply(&PersistDelta::Account(account(1))).unwrap();
    store.apply(&PersistDelta::Mail(cache(1, 10))).unwrap();
    let used = store.put_blob(b"used").unwrap();
    let unused = store.put_blob(b"unused body").unwrap();
    let keep: std::collections::BTreeSet<String> = [used.clone()].into_iter().collect();
    let blobs = BlobDir::new(dirs.blobs_dir());
    let tmp = blobs.path(&used).unwrap().with_extension("eml.tmp");
    std::fs::write(&tmp, b"half").unwrap();

    // Everything was written just now: a cutoff in the past spares it all.
    let spared = store.compact(&keep, std::time::UNIX_EPOCH).unwrap();
    assert_eq!(spared, app_io::Compacted::default());
    assert!(store.get_blob(&unused).unwrap().is_some());

    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    let done = store.compact(&keep, later).unwrap();
    assert_eq!(done.removed_blobs, 1);
    assert_eq!(
        done.freed_bytes,
        11 + 4,
        "the blob and the leftover temporary file"
    );
    assert!(store.get_blob(&unused).unwrap().is_none());
    assert_eq!(store.get_blob(&used).unwrap().unwrap(), b"used");
    assert!(!tmp.exists());
    // The database still loads after its compaction.
    assert!(store.load_snapshot().unwrap().is_some());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_cache_that_does_not_open_is_restored_from_the_daily_backup() {
    let dir = scratch("recover");
    let dirs = ProfileDirs::at(&dir);
    {
        let store = ProfileStore::open(&dirs).unwrap();
        store.apply(&PersistDelta::Account(account(1))).unwrap();
        store.apply(&PersistDelta::Mail(cache(1, 100))).unwrap();
        assert!(store.recovery().is_none());
    }
    // The first open backed up the empty cache; a day later the next open backs up both
    // files (here: that backup removed), then a crash tears the cache.
    std::fs::remove_dir_all(dirs.backups_dir()).unwrap();
    drop(ProfileStore::open(&dirs).unwrap());
    let backups = app_io::store::backup::list(&dirs).unwrap();
    assert_eq!(backups.len(), 1);
    assert!(backups[0].path.join("config.toml").is_file());
    std::fs::write(dirs.state_db(), b"not a redb file").unwrap();
    let store = ProfileStore::open(&dirs).unwrap();
    let recovery = store.recovery().unwrap();
    assert_eq!(recovery.restored_from.as_ref(), Some(&backups[0]));
    assert_eq!(std::fs::read(&recovery.broken).unwrap(), b"not a redb file");
    let state = store.load_snapshot().unwrap().unwrap();
    assert_eq!(state.messages.len(), 1, "the mail of the backup is back");
    std::fs::remove_dir_all(dir).unwrap();
}
