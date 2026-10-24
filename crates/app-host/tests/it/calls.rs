//! Kotlin's calls without the VM: JSON in and out, handles, errors instead of panics.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use app_host::{DESKTOP, Platform, calls};
use serde_json::Value;

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

/// The profiles `open` started the logs for.
static LOGGED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn record_logs(profile_dir: &Path) {
    LOGGED.lock().unwrap().push(profile_dir.to_owned());
}

/// The desktop, with logs that record where they were started.
const RECORDING: Platform = Platform {
    logging: record_logs,
    ..DESKTOP
};

#[test]
fn a_live_profile_through_the_calls() {
    let dir = std::env::temp_dir().join(format!("dail-host-calls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let config = serde_json::json!({ "profile_dir": dir }).to_string();
    let opened = json(&calls::open(
        RECORDING,
        &config,
        Arc::new(|| {}),
        Arc::new(|_| {}),
    ));
    let handle = i32::try_from(opened["handle"].as_i64().unwrap()).unwrap();
    assert!(handle >= 1);
    let configure = r#"{"event": "configure", "args": {"width_dp": 360, "height_dp": 800}}"#;
    let out = json(&calls::event(handle, configure));
    assert_eq!(out["frame"]["layout"]["class"], "compact");
    // The first frame tells the background schedule (the defaults: periodic, every 30 min).
    assert_eq!(
        out["externals"],
        serde_json::json!([{"external": "schedule", "args": {"sync": 1, "interval_mins": 30}}])
    );
    assert!(json(&calls::frame(handle))["frame"].is_object());
    assert_eq!(json(&calls::background(handle, r#""flush""#))["done"], true);
    assert_eq!(
        calls::blob(handle, r#"{"inline": {"blob": "x", "cid": "y"}}"#),
        None
    );
    assert!(json(&calls::event(handle, "not json"))["error"].is_string());
    assert!(json(&calls::background(handle, r#""nap""#))["error"].is_string());
    // A second host on the same profile is refused.
    let again = json(&calls::open(
        DESKTOP,
        &config,
        Arc::new(|| {}),
        Arc::new(|_| {}),
    ));
    assert!(again["error"].as_str().unwrap().contains("in use"));
    assert!(
        LOGGED.lock().unwrap().contains(&dir),
        "the platform's logs start"
    );
    calls::close(handle);
    assert!(json(&calls::frame(handle))["error"].is_string(), "closed");
    calls::close(handle);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_handles_and_bad_configs_are_errors() {
    for handle in [0, -1, 9999] {
        assert!(json(&calls::frame(handle))["error"].is_string());
        assert!(json(&calls::event(handle, r#"{"event": "tick"}"#))["error"].is_string());
        assert_eq!(calls::blob(handle, "{}"), None);
        calls::close(handle);
    }
    assert!(
        json(&calls::open(
            DESKTOP,
            "{}",
            Arc::new(|| {}),
            Arc::new(|_| {})
        ))["error"]
            .is_string()
    );
}
