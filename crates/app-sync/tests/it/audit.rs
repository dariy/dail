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
