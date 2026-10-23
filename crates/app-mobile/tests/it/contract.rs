//! The contract with the Kotlin app: every snapshot frame, one of every event and the JSON Schema
//! of `Frame` and `UiEvent` as fixtures under `android/app/src/test/resources/contract/`, which
//! the Kotlin types decode strictly. A stale, missing or extra fixture fails;
//! `DAIL_UPDATE_CONTRACT=1` rewrites them.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use app_core::{AppState, Command, Pane};
use app_mobile::event::Field;
use app_mobile::{External, Frame, UiEvent};

use crate::support::{Phone, SIZES, inbox_of};

/// The fixture directory, found through the run-time `CARGO_MANIFEST_DIR` (AGENTS.md §6).
fn dir() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from);
    manifest.join("../../android/app/src/test/resources/contract")
}

/// The frames of `frames.rs`, by fixture name.
fn frames() -> Vec<(String, Frame)> {
    let mut out = Vec::new();
    for (w, h) in SIZES {
        let mut phone = Phone::inbox(w, h);
        out.push((format!("inbox_{w}x{h}"), phone.frame()));
        let (_, threads) = inbox_of(phone.state());
        phone.dispatch(Command::OpenThread { id: threads[0] });
        out.push((format!("thread_{w}x{h}"), phone.frame()));
        let mut phone = Phone::inbox(w, h);
        phone.dispatch(Command::FocusPane {
            pane: Pane::Sidebar,
        });
        out.push((format!("drawer_{w}x{h}"), phone.frame()));
    }
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    let reply = phone.frame().reader.unwrap().actions;
    let reply = reply.iter().find(|b| b.label == "Reply").unwrap().action;
    phone.tap(reply);
    phone.event(UiEvent::Text {
        field: Field::Cc,
        text: "car".to_owned(),
    });
    out.push(("composer_reply".to_owned(), phone.frame()));
    let mut phone = Phone::new(AppState::default(), 360, 800);
    out.push(("no_accounts".to_owned(), phone.frame()));
    let about = phone.frame().top_bar.actions[0].action;
    phone.tap(about);
    out.push(("about".to_owned(), phone.frame()));
    out
}

/// One of every `UiEvent`; the `match` breaks the build when a variant is added.
fn events() -> Vec<UiEvent> {
    let all = vec![
        UiEvent::Configure {
            width_dp: 360,
            height_dp: 800,
            dark: true,
            font_scale: 1.25,
            locale: Some("en-US".to_owned()),
            keyboard: false,
        },
        UiEvent::Visible { visible: true },
        UiEvent::Tap { action: 7 },
        UiEvent::LongPress { action: 8 },
        UiEvent::Swipe { action: 9 },
        UiEvent::Scrolled {
            first: 20,
            count: 12,
        },
        UiEvent::Back,
        UiEvent::Refresh,
        UiEvent::Text {
            field: Field::Subject,
            text: "Lunch?".to_owned(),
        },
        UiEvent::Blur { field: Field::To },
        UiEvent::Key {
            chord: "Ctrl+Z".to_owned(),
        },
        UiEvent::Tick,
    ];
    for event in &all {
        match event {
            UiEvent::Configure { .. }
            | UiEvent::Visible { .. }
            | UiEvent::Tap { .. }
            | UiEvent::LongPress { .. }
            | UiEvent::Swipe { .. }
            | UiEvent::Scrolled { .. }
            | UiEvent::Back
            | UiEvent::Refresh
            | UiEvent::Text { .. }
            | UiEvent::Blur { .. }
            | UiEvent::Key { .. }
            | UiEvent::Tick => {}
        }
    }
    all
}

/// One of every `External`; the `match` breaks the build when a variant is added.
fn externals() -> Vec<External> {
    let all = vec![
        External::Haptic,
        External::PickFiles {
            draft: serde_json::from_str("\"00000000-0000-0000-0000-000000000007\"").unwrap(),
        },
    ];
    for external in &all {
        match external {
            External::Haptic | External::PickFiles { .. } => {}
        }
    }
    all
}

fn tag(value: &serde_json::Value, key: &str) -> String {
    value[key].as_str().unwrap().to_owned()
}

/// Every fixture, by path relative to [`dir`].
fn fixtures() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut put = |name: String, value: serde_json::Value| {
        let text = serde_json::to_string_pretty(&value).unwrap() + "\n";
        assert!(out.insert(name.clone(), text).is_none(), "{name} twice");
    };
    for (name, frame) in frames() {
        put(
            format!("frames/{name}.json"),
            serde_json::to_value(frame).unwrap(),
        );
    }
    for event in events() {
        let value = serde_json::to_value(event).unwrap();
        put(format!("events/{}.json", tag(&value, "event")), value);
    }
    for external in externals() {
        let value = serde_json::to_value(external).unwrap();
        put(format!("externals/{}.json", tag(&value, "external")), value);
    }
    put(
        "schema/frame.json".to_owned(),
        serde_json::to_value(schemars::schema_for!(Frame)).unwrap(),
    );
    put(
        "schema/ui_event.json".to_owned(),
        serde_json::to_value(schemars::schema_for!(UiEvent)).unwrap(),
    );
    out
}

/// The files under `dir`, relative to it.
fn on_disk(dir: &std::path::Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(groups) = std::fs::read_dir(dir) else {
        return out;
    };
    for group in groups {
        let group = group.unwrap().path();
        for file in std::fs::read_dir(&group).unwrap() {
            let path = file.unwrap().path();
            let rel = path
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(
                rel,
                std::fs::read_to_string(&path)
                    .unwrap()
                    .replace("\r\n", "\n"),
            );
        }
    }
    out
}

#[test]
fn the_contract_fixtures_are_current() {
    let dir = dir();
    let want = fixtures();
    if std::env::var_os("DAIL_UPDATE_CONTRACT").is_some_and(|v| v == "1") {
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        for (name, text) in &want {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        return;
    }
    let have = on_disk(&dir);
    let stale: Vec<_> = want
        .iter()
        .filter(|(name, text)| have.get(*name) != Some(text))
        .map(|(name, _)| name.as_str())
        .collect();
    let extra: Vec<_> = have.keys().filter(|n| !want.contains_key(*n)).collect();
    assert!(
        stale.is_empty() && extra.is_empty(),
        "contract fixtures in {} are out of date (stale or missing: {stale:?}; extra: {extra:?}); \
         rerun with DAIL_UPDATE_CONTRACT=1 and update the Kotlin types",
        dir.display()
    );
}

#[test]
fn the_events_cover_the_schema() {
    let schema = serde_json::to_value(schemars::schema_for!(UiEvent)).unwrap();
    let variants = schema["oneOf"].as_array().map_or(0, Vec::len);
    assert_eq!(events().len(), variants);
}
