//! `show_images_from` / `hide_images_from`: the per-sender setting for inline images.

use crate::support::ctx;
use app_core::{Command, CoreError, Effect, Engine, Origin, PersistDelta};
use serde_json::json;

fn show(sender: &str) -> Command {
    Command::ShowImagesFrom {
        sender: sender.to_owned(),
    }
}

fn hide(sender: &str) -> Command {
    Command::HideImagesFrom {
        sender: sender.to_owned(),
    }
}

fn senders(engine: &Engine) -> Vec<&str> {
    engine
        .state()
        .settings
        .images_from
        .iter()
        .map(String::as_str)
        .collect()
}

#[test]
fn showing_images_saves_the_sender_in_lower_case() {
    let mut engine = Engine::default();
    let outcome = engine
        .dispatch(show("  News@Example.ORG "), &ctx(1))
        .unwrap();
    assert_eq!(senders(&engine), ["news@example.org"]);
    assert_eq!(
        outcome.receipt.data,
        json!({"sender": "news@example.org", "showing": true, "changed": true})
    );
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Show images from news@example.org")
    );
    assert_eq!(
        outcome.effects,
        [Effect::PersistState {
            delta: PersistDelta::Settings(engine.state().settings.clone())
        }]
    );
    assert!(
        engine
            .state()
            .settings
            .shows_images_from("NEWS@example.org")
    );
    assert!(
        !engine
            .state()
            .settings
            .shows_images_from("other@example.org")
    );

    engine.dispatch(hide("news@example.org"), &ctx(2)).unwrap();
    assert!(senders(&engine).is_empty());
}

#[test]
fn repeating_it_changes_nothing_and_records_nothing() {
    let mut engine = Engine::default();
    engine.dispatch(show("a@x"), &ctx(1)).unwrap();
    let again = engine.dispatch(show("A@x"), &ctx(2)).unwrap();
    assert_eq!(again.receipt.data["changed"], false);
    assert!(again.effects.is_empty());
    assert!(again.receipt.undo_label.is_none());
    let absent = engine.dispatch(hide("b@x"), &ctx(3)).unwrap();
    assert_eq!(absent.receipt.data["changed"], false);
    // The one undo step is the first `show`.
    engine.dispatch(Command::Undo {}, &ctx(4)).unwrap();
    assert!(senders(&engine).is_empty());
    assert!(engine.dispatch(Command::Undo {}, &ctx(5)).is_err());
}

#[test]
fn senders_must_be_addresses() {
    let mut engine = Engine::default();
    for bad in [
        "",
        "news",
        "@example.org",
        "news@",
        "a b@x",
        "a@x\ny",
        &"a".repeat(400),
    ] {
        let err = engine.dispatch(show(bad), &ctx(1)).unwrap_err();
        assert!(
            matches!(&err, CoreError::InvalidArgument { field, .. } if field == "sender"),
            "{bad:?}: {err:?}"
        );
        assert!(matches!(
            engine.dispatch(hide(bad), &ctx(1)),
            Err(CoreError::InvalidArgument { .. })
        ));
    }
    assert!(senders(&engine).is_empty());
}

#[test]
fn undo_and_redo_round_trip() {
    let mut engine = Engine::default();
    engine.dispatch(show("a@x"), &ctx(1)).unwrap();
    engine.dispatch(show("b@x"), &ctx(2)).unwrap();
    engine.dispatch(hide("a@x"), &ctx(3)).unwrap();
    assert_eq!(senders(&engine), ["b@x"]);
    let undone = engine.dispatch(Command::Undo {}, &ctx(4)).unwrap();
    assert_eq!(undone.receipt.data["undone"], "Hide images from a@x");
    assert_eq!(senders(&engine), ["a@x", "b@x"]);
    engine.dispatch(Command::Undo {}, &ctx(5)).unwrap();
    assert_eq!(senders(&engine), ["a@x"]);
    engine.dispatch(Command::Redo {}, &ctx(6)).unwrap();
    engine.dispatch(Command::Redo {}, &ctx(7)).unwrap();
    assert_eq!(senders(&engine), ["b@x"]);
}

#[test]
fn agents_may_change_it() {
    let mut engine = Engine::default();
    let agent = app_core::Ctx {
        origin: Origin::Mcp,
        ..ctx(1)
    };
    engine.dispatch(show("a@x"), &agent).unwrap();
    assert_eq!(senders(&engine), ["a@x"]);
}
