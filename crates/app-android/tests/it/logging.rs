//! Android's logs on the desktop: the profile's `logs/android.log`.

use std::sync::Arc;

#[test]
fn opening_a_host_starts_the_log_file() {
    let dir = std::env::temp_dir().join(format!("dail-android-logs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let config = format!(r#"{{"profile_dir": {:?}}}"#, dir.display().to_string());
    let opened = app_host::calls::open(dail::PLATFORM, &config, Arc::new(|| {}), Arc::new(|_| {}));
    assert!(opened.contains("handle"), "{opened}");
    assert!(dir.join("logs").join("android.log").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
