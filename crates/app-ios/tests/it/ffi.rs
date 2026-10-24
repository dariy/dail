//! Swift's calls without Swift: the C exports over a live profile with no accounts, bad input,
//! freeing, and the log file.

// reason: these tests call the C exports, which are `unsafe extern "C"` by design (ADR 0010).
#![allow(unsafe_code)]
// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;

use dail::ffi::{self, DailListener};
use serde_json::Value;

/// Reads and frees an answer.
fn take(answer: *mut c_char) -> Value {
    assert!(!answer.is_null());
    // SAFETY: a string from a `dail_*` call, read before it is freed once.
    let text = unsafe { CStr::from_ptr(answer) }
        .to_str()
        .unwrap()
        .to_owned();
    // SAFETY: as above.
    unsafe { ffi::dail_string_free(answer) };
    serde_json::from_str(&text).unwrap()
}

fn c(text: &str) -> CString {
    CString::new(text).unwrap()
}

const SILENT: DailListener = DailListener {
    context: ptr::null_mut(),
    wake: None,
    alert: None,
};

extern "C" fn count_wake(context: *mut c_void) {
    // SAFETY: the test's `AtomicUsize`, alive until the host closes.
    let count = unsafe { &*context.cast::<std::sync::atomic::AtomicUsize>() };
    count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

#[test]
fn a_live_profile_through_the_c_calls() {
    let dir = std::env::temp_dir().join(format!("dail-ios-ffi-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let config = c(&serde_json::json!({ "profile_dir": dir }).to_string());
    let wakes = Box::new(std::sync::atomic::AtomicUsize::new(0));
    let listener = DailListener {
        context: ptr::from_ref(wakes.as_ref()).cast_mut().cast(),
        wake: Some(count_wake),
        alert: None,
    };
    // SAFETY: valid strings; the listener's context outlives the host.
    let opened = take(unsafe { ffi::dail_open(config.as_ptr(), listener) });
    let handle = i32::try_from(opened["handle"].as_i64().unwrap()).unwrap();
    let configure = c(r#"{"event": "configure", "args": {"width_dp": 820, "height_dp": 1180}}"#);
    // SAFETY: a valid string.
    let out = take(unsafe { ffi::dail_event(handle, configure.as_ptr()) });
    assert!(out["frame"].is_object(), "{out}");
    assert!(take(ffi::dail_frame(handle))["frame"].is_object());
    // SAFETY: valid strings.
    let done = take(unsafe { ffi::dail_background(handle, c(r#""flush""#).as_ptr()) });
    assert_eq!(done["done"], true);
    let request = c(r#"{"inline": {"blob": "x", "cid": "y"}}"#);
    // SAFETY: a valid string; the result is freed once.
    unsafe {
        let bytes = ffi::dail_blob(handle, request.as_ptr());
        assert!(bytes.ptr.is_null() && bytes.len == 0);
        ffi::dail_bytes_free(bytes);
    }
    // A second host on the same profile is refused.
    // SAFETY: a valid string.
    let again = take(unsafe { ffi::dail_open(config.as_ptr(), SILENT) });
    assert!(
        again["error"].as_str().unwrap().contains("in use"),
        "{again}"
    );
    assert!(dir.join("logs").join("ios.log").exists());
    ffi::dail_close(handle);
    assert!(take(ffi::dail_frame(handle))["error"].is_string(), "closed");
    drop(wakes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_input_is_an_error_not_a_crash() {
    // SAFETY: null and invalid UTF-8 are part of the contract.
    unsafe {
        assert!(take(ffi::dail_open(ptr::null(), SILENT))["error"].is_string());
        assert!(take(ffi::dail_open(c("{}").as_ptr(), SILENT))["error"].is_string());
        let latin1 = CString::from_vec_unchecked(vec![0xff, 0xfe]);
        assert!(take(ffi::dail_event(1, latin1.as_ptr()))["error"].is_string());
        assert!(take(ffi::dail_background(-1, ptr::null()))["error"].is_string());
        let none = ffi::dail_blob(0, ptr::null());
        assert!(none.ptr.is_null());
        ffi::dail_bytes_free(none);
        ffi::dail_string_free(ptr::null_mut());
    }
    assert!(take(ffi::dail_frame(0))["error"].is_string());
    ffi::dail_close(0);
}
