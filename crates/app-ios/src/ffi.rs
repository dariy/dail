//! The C calls Swift makes (`include/dail.h`): `app_host::calls` with C strings. This is the only
//! module that allows unsafe code, and it only converts arguments.
//!
//! Strings in are NUL-terminated UTF-8 that Swift owns for the duration of the call. Strings out
//! are owned by Rust until Swift hands them back to [`dail_string_free`]; bytes out likewise to
//! [`dail_bytes_free`]. A null or non-UTF-8 argument is answered with `{"error": "…"}`, never a
//! crash. The listener's functions are called from runtime threads; Swift hops to the main actor.

// reason: C exports need `#[unsafe(no_mangle)]`, and C strings, bytes and the listener's context
// are raw pointers (ADR 0010).
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::Arc;

use app_host::calls;

/// Swift's listener: `wake(context)` when a new frame is ready, `alert(context, json)` with each
/// notification (`json` lives for the call only). `context` must stay valid until `dail_close`,
/// and both functions must be callable from any thread.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DailListener {
    pub context: *mut c_void,
    pub wake: Option<extern "C" fn(context: *mut c_void)>,
    pub alert: Option<extern "C" fn(context: *mut c_void, json: *const c_char)>,
}

// SAFETY: the header requires both functions to be callable from any thread with `context`, and
// `context` to outlive the host; Rust never dereferences it.
unsafe impl Send for DailListener {}
// SAFETY: as for `Send`; the listener is only copied and called, never mutated.
unsafe impl Sync for DailListener {}

impl DailListener {
    fn wake(&self) {
        if let Some(wake) = self.wake {
            wake(self.context);
        }
    }

    fn alert(&self, json: String) {
        if let (Some(alert), Ok(json)) = (self.alert, CString::new(json)) {
            alert(self.context, json.as_ptr());
        }
    }
}

/// Bytes owned by Rust: `ptr` is null (and `len` 0) for none.
#[repr(C)]
pub struct DailBytes {
    pub ptr: *mut u8,
    pub len: usize,
}

fn error(message: &str) -> String {
    format!(r#"{{"error": "{message}"}}"#)
}

/// The UTF-8 string at `ptr`, if there is one.
///
/// # Safety
/// `ptr` is null or a NUL-terminated string that stays valid for the call.
unsafe fn read<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: non-null, and NUL-terminated and valid by the caller's contract.
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// `text` as a C string for Swift. JSON escapes NUL, so the fallback is never expected.
fn give(text: String) -> *mut c_char {
    CString::new(text)
        .or_else(|_| CString::new(error("the answer held a NUL")))
        .map_or(std::ptr::null_mut(), CString::into_raw)
}

/// Opens a host. `config` is `app_host::Config` as JSON. Answers `{"handle": n}` or `{"error"}`.
///
/// # Safety
/// `config` is null or a NUL-terminated string valid for the call; `listener` follows
/// [`DailListener`]'s contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dail_open(config: *const c_char, listener: DailListener) -> *mut c_char {
    // SAFETY: the caller's contract.
    let Some(config) = (unsafe { read(config) }) else {
        return give(error("config is not a UTF-8 string"));
    };
    // Methods, so that the closures capture the whole (`Send + Sync`) listener, not its fields.
    let wake: app_sync::Wake = Arc::new(move || listener.wake());
    let alert: app_host::Alert = Arc::new(move |json: String| listener.alert(json));
    give(calls::open(crate::PLATFORM, config, wake, alert))
}

/// One `UiEvent` as JSON. Answers `{"frame", "externals"}` or `{"error"}`.
///
/// # Safety
/// `event` is null or a NUL-terminated string valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dail_event(handle: i32, event: *const c_char) -> *mut c_char {
    // SAFETY: the caller's contract.
    match unsafe { read(event) } {
        Some(event) => give(calls::event(handle, event)),
        None => give(error("event is not a UTF-8 string")),
    }
}

/// The current frame. Answers `{"frame", "externals"}` or `{"error"}`.
#[unsafe(no_mangle)]
pub extern "C" fn dail_frame(handle: i32) -> *mut c_char {
    give(calls::frame(handle))
}

/// The bytes a `BlobRequest` (JSON) names, or none. Blocks; call it off the main thread.
///
/// # Safety
/// `request` is null or a NUL-terminated string valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dail_blob(handle: i32, request: *const c_char) -> DailBytes {
    // SAFETY: the caller's contract.
    let bytes = unsafe { read(request) }.and_then(|request| calls::blob(handle, request));
    match bytes {
        Some(bytes) => {
            let bytes = Box::into_raw(bytes.into_boxed_slice());
            DailBytes {
                len: bytes.len(),
                ptr: bytes.cast::<u8>(),
            }
        }
        None => DailBytes {
            ptr: std::ptr::null_mut(),
            len: 0,
        },
    }
}

/// A background task, an import or a debug task (JSON). Blocks; call it off the main thread.
///
/// # Safety
/// `task` is null or a NUL-terminated string valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dail_background(handle: i32, task: *const c_char) -> *mut c_char {
    // SAFETY: the caller's contract.
    match unsafe { read(task) } {
        Some(task) => give(calls::background(handle, task)),
        None => give(error("task is not a UTF-8 string")),
    }
}

/// Closes the host after its queued requests. The listener is not called afterwards.
#[unsafe(no_mangle)]
pub extern "C" fn dail_close(handle: i32) {
    calls::close(handle);
}

/// Frees a string a `dail_*` call returned. Null is ignored.
///
/// # Safety
/// `text` is null or a string from a `dail_*` call, not freed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dail_string_free(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: it came from `CString::into_raw` in `give`, once.
        drop(unsafe { CString::from_raw(text) });
    }
}

/// Frees bytes `dail_blob` returned. Null is ignored.
///
/// # Safety
/// `bytes` came from `dail_blob` and was not freed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dail_bytes_free(bytes: DailBytes) {
    if !bytes.ptr.is_null() {
        let slice = std::ptr::slice_from_raw_parts_mut(bytes.ptr, bytes.len);
        // SAFETY: it came from `Box::into_raw` of a boxed slice of this length in `dail_blob`, once.
        drop(unsafe { Box::from_raw(slice) });
    }
}
