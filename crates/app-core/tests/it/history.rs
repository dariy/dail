//! Undo / redo tests, including a property test over random setting sequences.

use app_core::{Command, CoreError, Ctx, Effect, Engine, Origin, PersistDelta, SettingKey};
use proptest::prelude::*;
use serde_json::json;

fn ctx() -> Ctx {
    Ctx {
        now_ms: 1_700_000_000_000,
        id_seed: 1,
        origin: Origin::Batch,
    }
}

fn set(key: SettingKey, value: u64) -> Command {
    Command::SetSetting {
        key,
        value: json!(value),
    }
}

#[test]
fn set_setting_mutates_persists_and_records_history() {
    let mut engine = Engine::default();
    let outcome = engine
        .dispatch(set(SettingKey::PollSecs, 300), &ctx())
        .unwrap();
    assert_eq!(engine.state().settings.poll_secs, 300);
    assert_eq!(outcome.receipt.undo_label.as_deref(), Some("Set poll_secs"));
    assert_eq!(outcome.receipt.data["previous"], 120);
    assert_eq!(
        outcome.effects,
        vec![Effect::PersistState {
            delta: PersistDelta::Settings(engine.state().settings.clone())
        }]
    );
    assert_eq!(engine.history().undo_len(), 1);
}

#[test]
fn set_setting_rejects_non_integer() {
    let mut engine = Engine::default();
    let cmd = Command::SetSetting {
        key: SettingKey::PollSecs,
        value: json!("fast"),
    };
    let err = engine.dispatch(cmd, &ctx()).unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "value"));
    assert_eq!(engine.state().settings.poll_secs, 120);
    assert_eq!(engine.history().undo_len(), 0);
}

#[test]
fn set_setting_rejects_out_of_range() {
    let mut engine = Engine::default();
    let err = engine
        .dispatch(set(SettingKey::HistoryCap, 0), &ctx())
        .unwrap_err();
    assert_eq!(
        serde_json::to_value(&err).unwrap(),
        json!({"error": "invalid_argument", "field": "value",
               "reason": "history_cap must be between 1 and 10000"})
    );
}

#[test]
fn undo_redo_round_trip() {
    let mut engine = Engine::default();
    let initial = engine.state().clone();
    engine
        .dispatch(set(SettingKey::UndoSendMs, 5_000), &ctx())
        .unwrap();
    let changed = engine.state().clone();

    let undo = engine.dispatch(Command::Undo {}, &ctx()).unwrap();
    assert_eq!(engine.state(), &initial);
    assert_eq!(undo.receipt.data, json!({"undone": "Set undo_send_ms"}));
    assert_eq!(undo.effects.len(), 1, "undo persists the reverted settings");

    let redo = engine.dispatch(Command::Redo {}, &ctx()).unwrap();
    assert_eq!(engine.state(), &changed);
    assert_eq!(redo.receipt.undo_label.as_deref(), Some("Set undo_send_ms"));
}

#[test]
fn nothing_to_undo_or_redo() {
    let mut engine = Engine::default();
    assert_eq!(
        engine.dispatch(Command::Undo {}, &ctx()).unwrap_err(),
        CoreError::NothingToUndo
    );
    assert_eq!(
        engine.dispatch(Command::Redo {}, &ctx()).unwrap_err(),
        CoreError::NothingToRedo
    );
}

#[test]
fn new_mutation_clears_redo() {
    let mut engine = Engine::default();
    engine
        .dispatch(set(SettingKey::PollSecs, 60), &ctx())
        .unwrap();
    engine.dispatch(Command::Undo {}, &ctx()).unwrap();
    engine
        .dispatch(set(SettingKey::PollSecs, 90), &ctx())
        .unwrap();
    assert_eq!(
        engine.dispatch(Command::Redo {}, &ctx()).unwrap_err(),
        CoreError::NothingToRedo
    );
}

#[test]
fn views_and_queries_are_not_recorded() {
    let mut engine = Engine::default();
    engine.dispatch(Command::OpenPalette {}, &ctx()).unwrap();
    engine.dispatch(Command::GetSettings {}, &ctx()).unwrap();
    assert_eq!(engine.history().undo_len(), 0);
}

#[test]
fn history_is_capped() {
    let mut engine = Engine::default();
    engine
        .dispatch(set(SettingKey::HistoryCap, 2), &ctx())
        .unwrap();
    for secs in [20, 30, 40] {
        engine
            .dispatch(set(SettingKey::PollSecs, secs), &ctx())
            .unwrap();
    }
    assert_eq!(engine.history().undo_len(), 2);
    engine.dispatch(Command::Undo {}, &ctx()).unwrap();
    engine.dispatch(Command::Undo {}, &ctx()).unwrap();
    assert_eq!(engine.state().settings.poll_secs, 20);
    assert_eq!(
        engine.dispatch(Command::Undo {}, &ctx()).unwrap_err(),
        CoreError::NothingToUndo
    );
}

fn setting_command() -> impl Strategy<Value = Command> {
    prop_oneof![
        (0_u64..=60_000).prop_map(|v| set(SettingKey::UndoSendMs, v)),
        (10_u64..=86_400).prop_map(|v| set(SettingKey::PollSecs, v)),
        (1_u64..=100_000).prop_map(|v| set(SettingKey::InitialWindow, v)),
        (50_u64..=10_000).prop_map(|v| set(SettingKey::HistoryCap, v)),
        Just(Command::OpenPalette {}),
        Just(Command::ClosePalette {}),
    ]
}

proptest! {
    #[test]
    fn undo_all_then_redo_all_restores_settings(cmds in prop::collection::vec(setting_command(), 0..40)) {
        let mut engine = Engine::default();
        let initial = engine.state().settings.clone();
        for cmd in cmds {
            engine.dispatch(cmd, &ctx()).unwrap();
        }
        let after = engine.state().settings.clone();
        let steps = engine.history().undo_len();
        for _ in 0..steps {
            engine.dispatch(Command::Undo {}, &ctx()).unwrap();
        }
        prop_assert_eq!(&engine.state().settings, &initial);
        for _ in 0..steps {
            engine.dispatch(Command::Redo {}, &ctx()).unwrap();
        }
        prop_assert_eq!(&engine.state().settings, &after);
    }
}
