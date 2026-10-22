//! Kotlin's six calls in safe Rust, JSON in and out: `jni.rs` only converts their arguments.
//!
//! A host is named by a handle, a small integer into a table (never a pointer: arm64 tags heap
//! pointers). Every call answers, `{"error": "…"}` on failure; a panic is caught here and
//! becomes such an error, so it never unwinds into the VM.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, PoisonError};

use app_sync::Wake;
use serde_json::{Value, json};

use crate::config::{BlobRequest, Config, Task};
use crate::error::HostError;
use crate::host::{BACKGROUND_LIMIT, Host};
use crate::logging;

/// The open hosts; handle `n` is slot `n - 1`.
static HOSTS: Mutex<Vec<Option<Arc<Host>>>> = Mutex::new(Vec::new());

fn error(message: impl std::fmt::Display) -> Value {
    json!({ "error": message.to_string() })
}

/// Runs `f`, turning a panic into an error.
fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown".to_owned());
        tracing::error!(%message, "panic in a host call");
        format!("panic: {message}")
    })
}

/// `guarded` for the calls that answer JSON.
fn answer(f: impl FnOnce() -> Result<Value, HostError>) -> String {
    match guarded(f) {
        Ok(Ok(value)) => value.to_string(),
        Ok(Err(err)) => error(err).to_string(),
        Err(panic) => error(panic).to_string(),
    }
}

fn host(handle: i32) -> Result<Arc<Host>, HostError> {
    let slot = usize::try_from(handle)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .ok_or(HostError::NoHost(handle))?;
    HOSTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(slot)
        .cloned()
        .flatten()
        .ok_or(HostError::NoHost(handle))
}

/// `open(config)`: `{"handle": n}`. Starts the logs on the first call.
pub fn open(config: &str, wake: Wake) -> String {
    answer(|| {
        let config: Config = serde_json::from_str(config)?;
        logging::init(&config.profile_dir);
        let host = Arc::new(Host::open(&config, wake)?);
        let mut hosts = HOSTS.lock().unwrap_or_else(PoisonError::into_inner);
        let slot = match hosts.iter().position(Option::is_none) {
            Some(slot) => {
                hosts[slot] = Some(host);
                slot
            }
            None => {
                hosts.push(Some(host));
                hosts.len() - 1
            }
        };
        let handle = i32::try_from(slot + 1).map_err(|_| HostError::NoHost(i32::MAX))?;
        Ok(json!({ "handle": handle }))
    })
}

/// `event(handle, event)`: `{"frame": …, "externals": […]}`.
pub fn event(handle: i32, event: &str) -> String {
    answer(|| Ok(serde_json::to_value(host(handle)?.event_json(event)?)?))
}

/// `frame(handle)`: `{"frame": …, "externals": […]}`.
pub fn frame(handle: i32) -> String {
    answer(|| Ok(serde_json::to_value(host(handle)?.frame())?))
}

/// `blob(handle, request)`: the bytes, or `None` (Kotlin's `null`).
pub fn blob(handle: i32, request: &str) -> Option<Vec<u8>> {
    guarded(|| {
        let request: BlobRequest = serde_json::from_str(request).ok()?;
        host(handle).ok()?.blob(&request)
    })
    .ok()
    .flatten()
}

/// `background(handle, task)`: `{"done": true}`, or `{"done": false}` after 3 minutes.
pub fn background(handle: i32, task: &str) -> String {
    answer(|| {
        let task: Task = serde_json::from_str(task)?;
        let done = host(handle)?.background(task, BACKGROUND_LIMIT)?;
        Ok(json!({ "done": done }))
    })
}

/// `close(handle)`: shuts the session down and frees the handle. An unknown handle is ignored.
pub fn close(handle: i32) {
    let closed = guarded(|| {
        let Ok(host) = host(handle) else {
            return;
        };
        host.close();
        let mut hosts = HOSTS.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(slot) = usize::try_from(handle - 1)
            .ok()
            .and_then(|n| hosts.get_mut(n))
        {
            *slot = None;
        }
    });
    if let Err(panic) = closed {
        tracing::error!(%panic, "close failed");
    }
}
