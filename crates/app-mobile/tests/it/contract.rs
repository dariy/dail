//! The contract with the Kotlin and Swift apps: every snapshot frame, one of every event and the
//! JSON Schema of `Frame` and `UiEvent` as fixtures under `android/app/src/test/resources/contract/`,
//! which the Kotlin types (`ContractTest`) and the Swift types (`ios/DailTests`) decode strictly. A stale, missing or extra fixture fails;
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
    // The folder tree in the iPad's portrait overlay drawer (MISC-10).
    let mut phone = crate::drawer::nested(820, 1180);
    phone.dispatch(Command::FocusPane {
        pane: Pane::Sidebar,
    });
    out.push(("drawer_tree".to_owned(), phone.frame()));
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::OpenPalette {});
    out.push(("palette".to_owned(), phone.frame()));
    phone.dispatch(Command::OpenThread { id: threads[0] });
    phone.dispatch(Command::ClosePalette {});
    let frame = phone.frame();
    let reader = frame.reader.unwrap();
    let mv = reader.actions.iter().find(|b| b.label == "Move…").unwrap();
    phone.tap(mv.action);
    out.push(("palette_step".to_owned(), phone.frame()));
    phone.event(UiEvent::Back);
    phone.event(UiEvent::Text {
        field: Field::Query,
        text: app_core::CommandName::ListMailboxes.spec().title.to_owned(),
    });
    let first = phone.frame().palette.unwrap().sections[0].rows[0].action;
    phone.tap(first);
    out.push(("result".to_owned(), phone.frame()));
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.bodies = Some(vec![crate::support::html_body(
        "Q3 numbers See the report",
        &crate::support::sample_document(),
    )]);
    phone.dispatch(Command::OpenThread { id: threads[0] });
    out.push(("reader_html".to_owned(), phone.frame()));
    let link = phone.frame().reader.unwrap().messages[0].links[0].action;
    phone.tap(link);
    out.push(("link".to_owned(), phone.frame()));
    let mut phone = crate::support::with_files();
    let images = phone.frame().reader.unwrap().messages[0]
        .images
        .clone()
        .unwrap();
    phone.tap(images.toggle.action);
    out.push(("attachments".to_owned(), phone.frame()));
    let open = phone.frame().reader.unwrap().messages[0].attachments[0].open;
    phone.tap(open);
    out.push(("preview".to_owned(), phone.frame()));
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::SetSearch {
        query: "lunch".to_owned(),
    });
    out.push(("search".to_owned(), phone.frame()));
    let mut phone = crate::updates::android_phone();
    out.push(("update_question".to_owned(), phone.frame()));
    phone.dispatch(app_front::updates::answer(true));
    crate::updates::with_release(&mut phone);
    out.push(("update_available".to_owned(), phone.frame()));
    let mut phone = Phone::new(AppState::default(), 360, 800);
    out.push(("no_accounts".to_owned(), phone.frame()));
    let about = phone.frame().top_bar.actions;
    let about = about.iter().find(|b| b.label == "About").unwrap().action;
    phone.tap(about);
    out.push(("about".to_owned(), phone.frame()));
    phone.event(UiEvent::Back);
    let custom = phone.frame().setup.unwrap().presets[2].action;
    phone.tap(custom);
    let next = phone.frame().setup.unwrap().actions[0].action;
    phone.tap(next);
    phone.event(UiEvent::Text {
        field: Field::Email,
        text: "me@example.org".to_owned(),
    });
    out.push(("setup_address".to_owned(), phone.frame()));
    for (name, section) in [
        ("settings_general", app_core::SettingsSection::General),
        ("settings_accounts", app_core::SettingsSection::Accounts),
    ] {
        let mut phone = Phone::inbox(360, 800);
        phone.dispatch(Command::OpenSettings {
            section: Some(section),
        });
        out.push((name.to_owned(), phone.frame()));
    }
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
        UiEvent::Pinch { zoom_in: true },
        UiEvent::Scrolled {
            first: 20,
            count: 12,
        },
        UiEvent::Back,
        UiEvent::Refresh,
        UiEvent::Online,
        UiEvent::Notification {
            act: app_mobile::NoteAct::OpenThread {
                thread: serde_json::from_str("\"00000000-0000-0000-0000-000000000009\"").unwrap(),
            },
        },
        UiEvent::Text {
            field: Field::Subject,
            text: "Lunch?".to_owned(),
        },
        UiEvent::Blur { field: Field::To },
        UiEvent::Key {
            chord: "Ctrl+Z".to_owned(),
        },
        UiEvent::KeyDown {
            code: 33,
            unicode: 101,
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
        },
        UiEvent::Tick,
        UiEvent::Share {
            mailto: Some("mailto:bob@example.org?subject=Hi".to_owned()),
            to: vec!["carol@example.org".to_owned()],
            subject: Some("Photos".to_owned()),
            text: Some("Here".to_owned()),
            files: vec!["content://com.example.files/a.pdf".to_owned()],
        },
    ];
    for event in &all {
        match event {
            UiEvent::Configure { .. }
            | UiEvent::Visible { .. }
            | UiEvent::Tap { .. }
            | UiEvent::LongPress { .. }
            | UiEvent::Swipe { .. }
            | UiEvent::Pinch { .. }
            | UiEvent::Scrolled { .. }
            | UiEvent::Back
            | UiEvent::Refresh
            | UiEvent::Online
            | UiEvent::Notification { .. }
            | UiEvent::Text { .. }
            | UiEvent::Blur { .. }
            | UiEvent::Key { .. }
            | UiEvent::KeyDown { .. }
            | UiEvent::Tick
            | UiEvent::Share { .. } => {}
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
        External::ImportFiles {
            draft: serde_json::from_str("\"00000000-0000-0000-0000-000000000007\"").unwrap(),
            uris: vec!["content://com.example.files/a.pdf".to_owned()],
        },
        External::Sending {
            draft: serde_json::from_str("\"00000000-0000-0000-0000-000000000007\"").unwrap(),
        },
        External::OpenUrl {
            url: "https://example.org/a".to_owned(),
        },
        External::Copy {
            text: "https://example.org/a".to_owned(),
        },
        External::SaveFiles {
            blob: "b0".to_owned(),
            files: vec![app_mobile::event::SaveFile {
                part: 2,
                name: "q3.pdf".to_owned(),
                mime: "application/pdf".to_owned(),
            }],
        },
        External::Schedule {
            sync: 1,
            interval_mins: 30,
        },
        External::SystemSettings {
            screen: app_mobile::event::SystemScreen::Battery,
        },
    ];
    for external in &all {
        match external {
            External::Haptic
            | External::PickFiles { .. }
            | External::ImportFiles { .. }
            | External::Sending { .. }
            | External::OpenUrl { .. }
            | External::Copy { .. }
            | External::SaveFiles { .. }
            | External::Schedule { .. }
            | External::SystemSettings { .. } => {}
        }
    }
    all
}

fn tag(value: &serde_json::Value, key: &str) -> String {
    value[key].as_str().unwrap().to_owned()
}

/// The running version lives in `Cargo.toml` only: a string that holds it (the About title) is
/// written with `<version>` in its place, so a release bump leaves the fixtures alone. Only a whole
/// string equal to the About title changes: a crate list's `x 0.1.0` stays.
fn unversioned(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) if *text == app_mobile::theme::about_title() => {
            *text = "About dail (<version>)".to_owned();
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(unversioned),
        serde_json::Value::Object(map) => map.values_mut().for_each(unversioned),
        _ => {}
    }
}

/// Every fixture, by path relative to [`dir`].
fn fixtures() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut put = |name: String, mut value: serde_json::Value| {
        unversioned(&mut value);
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
    for (name, note) in crate::background::samples() {
        put(
            format!("notifications/{name}.json"),
            serde_json::to_value(note).unwrap(),
        );
    }
    put(
        "schema/notification.json".to_owned(),
        serde_json::to_value(schemars::schema_for!(app_mobile::Notification)).unwrap(),
    );
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
