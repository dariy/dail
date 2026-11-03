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

/// STORE-10: the log rotates once it passes its limit, keeping one old file.
#[test]
fn the_log_rotates_at_its_limit() {
    use std::io::Write as _;
    let dir = std::env::temp_dir().join(format!("dail-android-rotate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("android.log");
    let mut log = dail::logging::Rotating::open(path.clone(), 100).unwrap();
    for n in 0..30 {
        writeln!(log, "line {n:02}").unwrap();
    }
    let old = std::fs::metadata(dir.join("android.log.1")).unwrap().len();
    let new = std::fs::metadata(&path).unwrap().len();
    assert!((100..=108).contains(&old), "{old}");
    assert!(new < 100, "{new}");
    let last = std::fs::read_to_string(&path).unwrap();
    assert!(last.ends_with("line 29\n"), "{last}");
    // A full file is rotated when it is opened, too.
    std::fs::write(&path, vec![b'x'; 200]).unwrap();
    drop(dail::logging::Rotating::open(path.clone(), 100).unwrap());
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}
