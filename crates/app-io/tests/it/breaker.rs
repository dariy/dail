//! The crash-loop breaker (STORE-7).

use app_io::breaker::Breaker;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-breaker-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_parse_that_ends_leaves_nothing() {
    let dir = scratch("ends");
    {
        let breaker = Breaker::open(&dir);
        drop(breaker.parsing("m1"));
    }
    let breaker = Breaker::open(&dir);
    assert!(!breaker.is_poisoned("m1"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_process_that_dies_mid_parse_poisons_that_message_for_good() {
    let dir = scratch("dies");
    {
        let breaker = Breaker::open(&dir);
        // The process "dies": the guard never drops.
        std::mem::forget(breaker.parsing("m2"));
        drop(breaker.parsing("m3"));
    }
    for _ in 0..2 {
        let breaker = Breaker::open(&dir);
        assert!(breaker.is_poisoned("m2"));
        assert!(!breaker.is_poisoned("m3"));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
