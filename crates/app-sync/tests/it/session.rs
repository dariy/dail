//! Session and executor tests against a recording in-test backend (no network).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_core::{AppState, Command, CoreError, Effect, Event, Origin, PersistDelta, SettingKey};
use app_sync::executor::run_effect;
use app_sync::{Backend, BoxFuture, Clock, CounterSeeds, Session, SyncError, VirtualClock};
use serde_json::json;

#[derive(Default)]
struct RecordingBackend {
    initial: Option<AppState>,
    fail_with: Option<String>,
    executed: Mutex<Vec<Effect>>,
}

impl RecordingBackend {
    fn executed(&self) -> Vec<Effect> {
        self.executed.lock().unwrap().clone()
    }
}

impl Backend for RecordingBackend {
    fn name(&self) -> &'static str {
        "recording"
    }

    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>> {
        Box::pin(async move { Ok(self.initial.clone()) })
    }

    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>> {
        Box::pin(async move {
            // Make ordering bugs visible: later effects would overtake earlier ones.
            tokio::time::sleep(Duration::from_millis(10)).await;
            let name = effect.name();
            self.executed.lock().unwrap().push(effect);
            match &self.fail_with {
                Some(reason) => Err(SyncError::Effect {
                    effect: name,
                    reason: reason.clone(),
                }),
                None => Ok(vec![Event::Persisted {}]),
            }
        })
    }
}

async fn open(backend: Arc<RecordingBackend>) -> Session {
    Session::open(
        backend,
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap()
}

fn set(key: SettingKey, value: u64) -> Command {
    Command::SetSetting {
        key,
        value: json!(value),
    }
}

#[tokio::test(start_paused = true)]
async fn submit_dispatches_and_runs_effects_in_order() {
    let backend = Arc::new(RecordingBackend::default());
    let session = open(Arc::clone(&backend)).await;
    let receipt = session
        .submit(set(SettingKey::PollSecs, 300), Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.name, "set_setting");
    session
        .submit(Command::Undo {}, Origin::Cli)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();

    let persisted: Vec<Option<u64>> = backend
        .executed()
        .into_iter()
        .map(|effect| match effect {
            Effect::PersistState {
                delta: PersistDelta::Settings(settings),
            } => Some(settings.poll_secs),
            _ => None,
        })
        .collect();
    assert_eq!(
        persisted,
        vec![Some(300), Some(120)],
        "store writes must keep order"
    );
    assert_eq!(session.state().settings.poll_secs, 120);
    session.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn rejected_command_returns_core_error() {
    let session = open(Arc::new(RecordingBackend::default())).await;
    let result = session.submit(Command::Undo {}, Origin::Mcp).await.unwrap();
    assert_eq!(result, Err(CoreError::NothingToUndo));
}

#[tokio::test(start_paused = true)]
async fn failed_effect_surfaces_in_state() {
    let backend = Arc::new(RecordingBackend {
        fail_with: Some("disk full".into()),
        ..RecordingBackend::default()
    });
    let session = open(backend).await;
    let changes = session.subscribe();
    session
        .submit(set(SettingKey::PollSecs, 60), Origin::Gui)
        .await
        .unwrap()
        .unwrap();
    session.flush().await.unwrap();
    let error = session.state().status.persist_error.clone().unwrap();
    assert!(error.contains("disk full"), "{error}");
    assert!(changes.has_changed().unwrap());
}

#[tokio::test(start_paused = true)]
async fn open_loads_persisted_state() {
    let mut initial = AppState::default();
    initial.settings.poll_secs = 999;
    let backend = Arc::new(RecordingBackend {
        initial: Some(initial),
        ..RecordingBackend::default()
    });
    let session = open(backend).await;
    assert_eq!(session.state().settings.poll_secs, 999);
}

#[tokio::test(start_paused = true)]
async fn shutdown_waits_for_in_flight_effects() {
    let backend = Arc::new(RecordingBackend::default());
    let session = open(Arc::clone(&backend)).await;
    session
        .submit(set(SettingKey::InitialWindow, 50), Origin::Batch)
        .await
        .unwrap()
        .unwrap();
    session.shutdown().await.unwrap();
    assert_eq!(backend.executed().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn schedule_tick_fires_on_the_virtual_clock() {
    let backend = RecordingBackend::default();
    let clock = VirtualClock::starting_at(1_000);
    let at_ms = 11_000;
    let events = run_effect(&backend, &clock, Effect::ScheduleTick { at_ms }).await;
    assert_eq!(events, vec![Event::Tick { now_ms: at_ms }]);
    assert_eq!(clock.now_ms(), at_ms);
    assert!(
        backend.executed().is_empty(),
        "timers never reach the backend"
    );
}

#[tokio::test(start_paused = true)]
async fn ingested_events_change_state() {
    let session = open(Arc::new(RecordingBackend::default())).await;
    session
        .ingest(Event::PersistFailed {
            error: "boom".into(),
        })
        .await
        .unwrap();
    session.flush().await.unwrap();
    assert_eq!(
        session.state().status.persist_error.as_deref(),
        Some("boom")
    );
}

/// Verifies accounts; the one whose login is `slow@example.org` hangs until released.
#[derive(Default)]
struct GatedBackend {
    release: tokio::sync::Notify,
}

impl Backend for GatedBackend {
    fn name(&self) -> &'static str {
        "gated"
    }

    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>> {
        Box::pin(async { Ok(None) })
    }

    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>> {
        Box::pin(async move {
            match effect {
                Effect::VerifyAccount { account, login, .. } => {
                    if login == "slow@example.org" {
                        self.release.notified().await;
                    }
                    Ok(vec![Event::AccountVerified { account }])
                }
                _ => Ok(vec![Event::Persisted {}]),
            }
        })
    }
}

#[tokio::test]
async fn a_slow_server_delays_only_its_own_account() {
    let backend = Arc::new(GatedBackend::default());
    let session = Session::open(
        Arc::clone(&backend) as Arc<dyn Backend>,
        Arc::new(app_sync::SystemClock),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap();
    let mut ids = Vec::new();
    for email in ["slow@example.org", "fast@example.org"] {
        let receipt = session
            .submit(
                Command::AddAccount {
                    preset: app_core::ProviderPreset::Custom,
                    email: email.into(),
                    display_name: None,
                    password: None,
                    imap: Some(app_core::Endpoint::parse("imap.example.org:993").unwrap()),
                    smtp: Some(app_core::Endpoint::parse("smtp.example.org:465").unwrap()),
                },
                Origin::Cli,
            )
            .await
            .unwrap()
            .unwrap();
        ids.push(
            serde_json::from_value::<app_core::AccountId>(receipt.data["id"].clone()).unwrap(),
        );
    }
    for id in &ids {
        session
            .submit(Command::TestAccount { id: *id }, Origin::Cli)
            .await
            .unwrap()
            .unwrap();
    }
    let verified = |session: &Session, id| {
        matches!(
            session.state().accounts[&id].status,
            app_core::AccountStatus::Verified { .. }
        )
    };
    // The fast account is verified although the slow one's server has not answered.
    let mut fast = false;
    for _ in 0..200 {
        if verified(&session, ids[1]) {
            fast = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(fast, "the fast account waited for the slow one");
    assert!(!verified(&session, ids[0]));
    backend.release.notify_one();
    session.flush().await.unwrap();
    assert!(verified(&session, ids[0]));
    session.shutdown().await.unwrap();
}
