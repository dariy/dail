//! `pull_settings` / `push_settings` and the settings folder (ADR 0015, SET-2).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, T0, ctx, names};
use app_core::{
    AccountId, Command, CoreError, Effect, Event, MailboxRole, Origin, ProviderPreset,
    RemoteMailbox, SETTINGS_MAX_BYTES, SettingsCopy, Stamped, SyncedSettings,
};

const COPY: SettingsCopy = SettingsCopy {
    uid_validity: 9,
    uid: 4,
};

fn with_folder() -> Mail {
    Mail::new(
        ProviderPreset::Custom,
        &[
            ("INBOX", Some(MailboxRole::Inbox)),
            ("dail", None),
            ("dail/Settings", None),
        ],
    )
}

fn remote_doc(key: &str, updated_ms: i64, value: i64) -> String {
    let mut doc = SyncedSettings::new();
    doc.updated_ms = updated_ms;
    doc.prefs.insert(
        key.into(),
        Stamped {
            updated_ms,
            value: value.into(),
        },
    );
    String::from_utf8(doc.to_json().unwrap()).unwrap()
}

fn read(m: &mut Mail, copy: Option<SettingsCopy>, document: Option<String>) {
    let account = m.account;
    m.feed(Event::SettingsRead {
        account,
        copy,
        document,
    });
}

#[test]
fn the_settings_folder_is_hidden_and_pulled_on_listing() {
    let m = with_folder();
    let paths: Vec<&str> = m
        .engine
        .state()
        .mailboxes
        .values()
        .map(|mb| mb.path.as_str())
        .collect();
    assert_eq!(paths, ["INBOX"], "dail and dail/Settings are not mailboxes");
    assert!(m.engine.state().synced.pulling);
}

#[test]
fn a_parent_with_other_folders_stays() {
    let m = Mail::new(
        ProviderPreset::Custom,
        &[
            ("INBOX", None),
            ("dail", None),
            ("dail/Settings", None),
            ("dail/Mine", None),
        ],
    );
    let mut paths: Vec<&str> = m
        .engine
        .state()
        .mailboxes
        .values()
        .map(|mb| mb.path.as_str())
        .collect();
    paths.sort_unstable();
    assert_eq!(paths, ["INBOX", "dail", "dail/Mine"]);
}

#[test]
fn listing_emits_the_read_and_without_the_folder_counts_as_pulled() {
    let mut engine = app_core::Engine::default();
    engine
        .dispatch(
            Command::AddAccount {
                preset: ProviderPreset::Custom,
                email: "me@example.com".into(),
                display_name: None,
                password: None,
                imap: Some(app_core::Endpoint {
                    host: "imap.example.com".into(),
                    port: 993,
                    security: app_core::Security::Tls,
                }),
                smtp: Some(app_core::Endpoint {
                    host: "smtp.example.com".into(),
                    port: 465,
                    security: app_core::Security::Tls,
                }),
            },
            &ctx(1),
        )
        .unwrap();
    let account = AccountId::from_uuid(uuid::Uuid::from_u128(1 << 64));
    let listed = |path: &str| RemoteMailbox {
        path: path.into(),
        delimiter: Some(".".into()),
        special_use: None,
        selectable: true,
    };
    let out = engine
        .ingest(
            Event::MailboxesListed {
                account,
                mailboxes: vec![listed("INBOX")],
            },
            &ctx(2),
        )
        .unwrap();
    assert!(!names(&out.effects).contains(&"read_settings_blob"));
    assert_eq!(engine.state().synced.pulled_ms, Some(T0 + 2000));

    let out = engine
        .ingest(
            Event::MailboxesListed {
                account,
                mailboxes: vec![listed("INBOX"), listed("dail.Settings")],
            },
            &ctx(3),
        )
        .unwrap();
    let reads: Vec<_> = out
        .effects
        .iter()
        .filter_map(|e| match e {
            Effect::ReadSettingsBlob { path, known, .. } => Some((path.clone(), *known)),
            _ => None,
        })
        .collect();
    assert_eq!(reads, [("dail.Settings".to_owned(), None)]);
}

#[test]
fn a_pull_merges_the_server_copy_and_remembers_it() {
    let mut m = with_folder();
    read(&mut m, Some(COPY), Some(remote_doc("list_density", 50, 2)));
    let synced = &m.engine.state().synced;
    assert!(!synced.pulling);
    assert_eq!(synced.copy, Some(COPY));
    assert_eq!(synced.source, Some(m.account));
    assert_eq!(synced.error, None);
    assert_eq!(synced.settings.prefs["list_density"].value, 2);

    // The next pull names the copy it knows, so it is not downloaded again.
    let out = m.run(Command::PullSettings { account: None }).unwrap();
    assert!(matches!(
        out.effects.as_slice(),
        [Effect::ReadSettingsBlob { known: Some(k), path, .. }] if *k == COPY && path == "dail/Settings"
    ));
    // A second pull while one runs starts nothing.
    let out = m.run(Command::PullSettings { account: None }).unwrap();
    assert!(out.effects.is_empty());
    assert_eq!(out.receipt.data["started"], false);
    // Unchanged: nothing to merge, the document stays.
    read(&mut m, Some(COPY), None);
    assert_eq!(
        m.engine.state().synced.settings.prefs["list_density"].value,
        2
    );
}

#[test]
fn an_older_server_copy_loses_field_by_field() {
    let mut m = with_folder();
    read(&mut m, Some(COPY), Some(remote_doc("list_density", 50, 2)));
    let older = SettingsCopy { uid: 5, ..COPY };
    m.run(Command::PullSettings { account: None }).unwrap();
    read(&mut m, Some(older), Some(remote_doc("list_density", 10, 7)));
    assert_eq!(
        m.engine.state().synced.settings.prefs["list_density"].value,
        2
    );
}

#[test]
fn invalid_and_oversized_copies_are_ignored_with_an_error() {
    let mut m = with_folder();
    read(&mut m, Some(COPY), Some(remote_doc("list_density", 50, 2)));
    let before = m.engine.state().synced.settings.clone();

    m.run(Command::PullSettings { account: None }).unwrap();
    read(
        &mut m,
        Some(SettingsCopy { uid: 5, ..COPY }),
        Some("{not json".into()),
    );
    let synced = &m.engine.state().synced;
    assert_eq!(synced.settings, before);
    assert!(synced.error.as_ref().unwrap().error.starts_with("ERR021: "));
    assert_eq!(
        synced.copy.unwrap().uid,
        5,
        "the bad copy is not read again"
    );

    m.run(Command::PullSettings { account: None }).unwrap();
    let huge = format!(
        "{{\"prefs\":{{}},\"pad\":\"{}\"}}",
        "x".repeat(SETTINGS_MAX_BYTES)
    );
    read(&mut m, Some(SettingsCopy { uid: 6, ..COPY }), Some(huge));
    let synced = &m.engine.state().synced;
    assert_eq!(synced.settings, before);
    assert!(synced.error.as_ref().unwrap().error.starts_with("ERR020: "));

    m.run(Command::PullSettings { account: None }).unwrap();
    let invalid = r#"{"schema":1,"rules":{"00000000-0000-0000-0000-000000000001":{"updated_ms":1,"rule":{"name":"r","when":{"test":"all","of":[]},"then":[]}}}}"#;
    read(
        &mut m,
        Some(SettingsCopy { uid: 7, ..COPY }),
        Some(invalid.into()),
    );
    let synced = &m.engine.state().synced;
    assert!(synced.error.as_ref().unwrap().error.starts_with("ERR022: "));

    // A good copy clears the error.
    m.run(Command::PullSettings { account: None }).unwrap();
    read(&mut m, Some(SettingsCopy { uid: 8, ..COPY }), None);
    assert_eq!(m.engine.state().synced.error, None);
}

#[test]
fn push_writes_the_merged_document_and_reads_it_back() {
    let mut m = with_folder();
    read(&mut m, Some(COPY), Some(remote_doc("list_density", 50, 2)));
    let account = m.account;
    let out = m.run(Command::PushSettings {}).unwrap();
    let Some(Effect::WriteSettingsBlob {
        path,
        document,
        account: to,
        ..
    }) = out.effects.first()
    else {
        panic!("no write: {:?}", out.effects);
    };
    assert_eq!((path.as_str(), *to), ("dail/Settings", account));
    let written = SyncedSettings::from_json(document.as_bytes()).unwrap();
    assert_eq!(written.prefs["list_density"].value, 2);
    assert!(m.engine.state().synced.pushing);
    assert!(matches!(
        m.run(Command::PushSettings {}),
        Err(CoreError::InvalidArgument { .. })
    ));

    let out = m.feed(Event::SettingsWritten { account });
    assert!(!m.engine.state().synced.pushing);
    assert!(m.engine.state().synced.pushed_ms.is_some());
    assert_eq!(names(&out.effects), ["read_settings_blob"]);
}

#[test]
fn failures_land_in_synced_error() {
    let mut m = with_folder();
    let account = m.account;
    m.feed(Event::SettingsFailed {
        account,
        write: false,
        error: "IMAP EXAMINE dail/Settings failed: boom".into(),
    });
    let synced = &m.engine.state().synced;
    assert!(!synced.pulling);
    assert!(synced.error.as_ref().unwrap().error.contains("boom"));
    assert!(synced.pulled_ms.is_none());

    read(&mut m, None, None);
    m.run(Command::PushSettings {}).unwrap();
    m.feed(Event::SettingsFailed {
        account,
        write: true,
        error: "IMAP APPEND failed".into(),
    });
    assert!(!m.engine.state().synced.pushing);
    // The failure event of each effect reports which one failed.
    let read_effect = Effect::ReadSettingsBlob {
        account,
        imap: m.engine.state().accounts[&account].imap_login(),
        path: "dail/Settings".into(),
        known: None,
    };
    assert!(matches!(
        read_effect.failure_event("x".into()),
        Event::SettingsFailed { write: false, .. }
    ));
}

#[test]
fn errors_of_the_commands() {
    let mut empty = app_core::Engine::default();
    assert!(matches!(
        empty.dispatch(Command::PullSettings { account: None }, &ctx(1)),
        Err(CoreError::InvalidArgument { .. })
    ));
    assert!(matches!(
        empty.dispatch(Command::PushSettings {}, &ctx(1)),
        Err(CoreError::InvalidArgument { .. })
    ));
    let mut m = with_folder();
    let unknown = AccountId::from_uuid(uuid::Uuid::from_u128(77));
    assert!(matches!(
        m.run(Command::PullSettings {
            account: Some(unknown)
        }),
        Err(CoreError::NotFound { .. })
    ));
    // No pull has finished yet: a push could overwrite a newer server copy.
    assert!(matches!(
        m.run(Command::PushSettings {}),
        Err(CoreError::InvalidArgument { .. })
    ));
}

#[test]
fn agents_may_pull_but_not_push() {
    let mut m = with_folder();
    read(&mut m, Some(COPY), None);
    let mcp = |n| app_core::Ctx {
        origin: Origin::Mcp,
        ..ctx(n)
    };
    m.engine
        .dispatch(Command::PullSettings { account: None }, &mcp(40))
        .unwrap();
    assert!(matches!(
        m.engine.dispatch(Command::PushSettings {}, &mcp(41)),
        Err(CoreError::OriginNotAllowed { .. })
    ));
}

#[test]
fn neither_command_is_recorded_in_history() {
    let mut m = with_folder();
    read(&mut m, Some(COPY), None);
    let undo = m.engine.history().undo_len();
    let out = m.run(Command::PushSettings {}).unwrap();
    assert_eq!(out.receipt.undo_label, None);
    assert_eq!(m.engine.history().undo_len(), undo);
}
