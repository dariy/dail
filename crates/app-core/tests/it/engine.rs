//! Headless engine tests: commands, events and error paths.

use app_core::{
    AppState, Command, CommandKind, CoreError, Ctx, Engine, EntityRef, Event, Origin, Settings,
};
use serde_json::json;

fn ctx() -> Ctx {
    Ctx {
        now_ms: 1_700_000_000_000,
        id_seed: 7,
        origin: Origin::Cli,
    }
}

#[test]
fn get_settings_returns_settings_without_mutating() {
    let mut engine = Engine::default();
    let before = engine.state().clone();
    let outcome = engine.dispatch(Command::GetSettings {}, &ctx()).unwrap();
    assert_eq!(outcome.receipt.kind, Some(CommandKind::Query));
    assert_eq!(
        outcome.receipt.data,
        serde_json::to_value(Settings::default()).unwrap()
    );
    assert!(outcome.effects.is_empty());
    assert_eq!(engine.state(), &before);
}

#[test]
fn palette_view_commands_toggle_view_state() {
    let mut engine = Engine::default();
    let outcome = engine.dispatch(Command::OpenPalette {}, &ctx()).unwrap();
    assert!(engine.state().view.palette_open);
    assert_eq!(outcome.receipt.kind, Some(CommandKind::View));
    assert_eq!(outcome.receipt.affected, vec![EntityRef::View]);
    engine.dispatch(Command::ClosePalette {}, &ctx()).unwrap();
    assert!(!engine.state().view.palette_open);
}

#[test]
fn persist_events_surface_in_status() {
    let mut engine = Engine::default();
    engine
        .ingest(
            Event::PersistFailed {
                error: "disk full".into(),
            },
            &ctx(),
        )
        .unwrap();
    assert_eq!(
        engine.state().status.persist_error.as_deref(),
        Some("disk full")
    );
    let outcome = engine.ingest(Event::Persisted {}, &ctx()).unwrap();
    assert_eq!(outcome.receipt.kind, None);
    assert_eq!(outcome.receipt.name, "persisted");
    assert_eq!(engine.state().status.persist_error, None);
}

#[test]
fn tick_does_not_change_state() {
    let mut engine = Engine::default();
    let outcome = engine.ingest(Event::Tick { now_ms: 5 }, &ctx()).unwrap();
    assert!(outcome.effects.is_empty());
    assert_eq!(engine.state(), &AppState::default());
}

#[test]
fn from_value_accepts_missing_args() {
    let cmd = Command::from_value(json!({"cmd": "open_palette"})).unwrap();
    assert_eq!(cmd, Command::OpenPalette {});
    let cmd = Command::from_value(json!({"cmd": "open_palette", "args": {}})).unwrap();
    assert_eq!(cmd, Command::OpenPalette {});
}

#[test]
fn from_value_rejects_unknown_command() {
    let err = Command::from_value(json!({"cmd": "launch_rockets"})).unwrap_err();
    assert!(matches!(err, CoreError::InvalidCommand { .. }));
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["error"], "invalid_command");
}

#[test]
fn commands_serialize_with_cmd_and_args() {
    let json = serde_json::to_value(Command::ClosePalette {}).unwrap();
    assert_eq!(json, json!({"cmd": "close_palette", "args": {}}));
}
