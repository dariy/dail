//! The audit log: a session's inputs as redacted replay records that replay to the same state.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use app_core::{
    Addr, AppState, Command, Effect, Engine, Event, Origin, ProviderPreset, Secret, SettingKey,
    replay,
};
use app_sync::{AuditLog, Backend, BoxFuture, CounterSeeds, Session, SyncError, VirtualClock};
use serde_json::json;

struct NullBackend;

impl Backend for NullBackend {
    fn name(&self) -> &'static str {
        "null"
    }

    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>> {
        Box::pin(async { Ok(None) })
    }

    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>> {
        Box::pin(async move {
            Ok(match effect {
                Effect::PersistState { .. } => vec![Event::Persisted {}],
                _ => Vec::new(),
            })
        })
    }
}

#[tokio::test]
async fn every_input_is_logged_redacted_and_replays_to_the_same_state() {
    let dir = std::env::temp_dir().join(format!("dail-audit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let log = AuditLog::new(dir.join("logs"));
    let session = Session::open_audited(
        Arc::new(NullBackend),
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
        Some(log.clone()),
    )
    .await
    .unwrap();
    let add = session
        .submit(
            Command::AddAccount {
                preset: ProviderPreset::Gmail,
                email: "me@gmail.com".into(),
                display_name: None,
                password: Some(Secret::new("hunter2")),
                imap: None,
                smtp: None,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    let account = serde_json::from_value(add.data["id"].clone()).unwrap();
    for (cmd, origin) in [
        (
            Command::SetSetting {
                key: SettingKey::PollSecs,
                value: json!(300),
            },
            Origin::Gui,
        ),
        (
            Command::CreateDraft {
                account,
                to: vec![Addr {
                    name: None,
                    email: "bob@example.org".into(),
                }],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Plans".into(),
                body: "my private text".into(),
            },
            Origin::Mcp,
        ),
        (Command::Redo {}, Origin::Batch),
    ] {
        let _ = session.submit(cmd, origin).await.unwrap();
    }
    session.flush().await.unwrap();
    let live = session.state();
    session.shutdown().await.unwrap();

    let text = std::fs::read_to_string(log.file_for(1_700_000_000_000)).unwrap();
    assert!(text.starts_with("# session opened at "), "{text}");
    assert!(!text.contains("hunter2"), "no password");
    assert!(!text.contains("my private text"), "no message text");
    let records = replay::parse_log(&text).unwrap();
    let origins: Vec<&str> = records.iter().map(|(_, r)| r.origin.as_str()).collect();
    assert_eq!(
        &origins[..2],
        ["cli", "runtime"],
        "commands and events: {origins:?}"
    );
    assert!(origins.contains(&"mcp") && origins.contains(&"batch"));

    let mut engine = Engine::default();
    let steps = replay::run(&mut engine, records);
    let rejected = steps
        .iter()
        .filter(|step| matches!(step.result, app_core::StepResult::Err(_)))
        .count();
    assert_eq!(rejected, 1, "the rejected redo is rejected again");
    let mut replayed = engine.state().clone();
    // Only the redacted text differs.
    for (draft, original) in replayed.drafts.values_mut().zip(live.drafts.values()) {
        assert_eq!(draft.body_text, "<redacted>");
        draft.body_text.clone_from(&original.body_text);
    }
    assert_eq!(&replayed, live.as_ref());
}

/// STORE-10: day files older than the kept days are swept; today's, recent ones and other files
/// stay.
#[test]
fn old_replay_logs_are_swept() {
    let dir = std::env::temp_dir().join(format!("dail-audit-sweep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = AuditLog::new(&dir);
    // 2026-11-03 12:00 UTC.
    let now = 1_793_707_200_000;
    for name in [
        "replay-20261103.jsonl",
        "replay-20261020.jsonl",
        "replay-20261019.jsonl",
        "replay-20250101.jsonl",
        "android.log",
        "replay-notes.jsonl",
    ] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    assert_eq!(log.sweep(now, 14), 2);
    let mut left: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            "android.log",
            "replay-20261020.jsonl",
            "replay-20261103.jsonl",
            "replay-notes.jsonl"
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}
