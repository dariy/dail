//! The JNI exports of `net.darii.dail.Native` (Kotlin: `@JvmStatic external fun`), and the
//! listener's callbacks. Only argument conversion lives here; the calls are in `calls.rs`.
//!
//! ```kotlin
//! object Native {
//!     @JvmStatic external fun open(config: String, listener: Listener, context: Context): String
//!     @JvmStatic external fun event(handle: Int, event: String): String
//!     @JvmStatic external fun frame(handle: Int): String
//!     @JvmStatic external fun blob(handle: Int, request: String): ByteArray?
//!     @JvmStatic external fun background(handle: Int, task: String): String
//!     @JvmStatic external fun close(handle: Int)
//! }
//! interface Listener { fun wake(); fun alert(json: String) }
//! ```

// reason: JNI exports need `#[unsafe(export_name)]`, and `JNI_OnLoad` and ndk-context take raw
// pointers (ADR 0009).
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::Arc;

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::refs::Global;
use jni::sys::{JNI_VERSION_1_6, jint};
use jni::vm::JavaVM;
use jni::{EnvUnowned, jni_sig, jni_str};

use crate::calls;

/// Called by the VM when `System.loadLibrary("dail")` loads the library: keeps the VM for the
/// threads that call back.
#[unsafe(export_name = "JNI_OnLoad")]
pub extern "system" fn jni_on_load(vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jint {
    if !vm.is_null() {
        // SAFETY: the VM passes a valid `JavaVM` pointer that lives as long as the process.
        let _ = unsafe { JavaVM::from_raw(vm) };
    }
    JNI_VERSION_1_6
}

/// Hands ndk-context the VM and the application context (for keyring's Android store), once.
#[cfg(target_os = "android")]
fn android_context(env: &mut jni::Env<'_>, context: &JObject<'_>) -> jni::errors::Result<()> {
    static CONTEXT: std::sync::OnceLock<Global<JObject<'static>>> = std::sync::OnceLock::new();
    if CONTEXT.get().is_some() {
        return Ok(());
    }
    let global = env.new_global_ref(context)?;
    let vm = env.get_java_vm()?;
    let raw = global.as_raw();
    if CONTEXT.set(global).is_ok() {
        // SAFETY: the VM pointer lives as long as the process, and the context is a global
        // reference kept in `CONTEXT` for good; this runs once.
        unsafe { ndk_context::initialize_android_context(vm.get_raw().cast(), raw.cast()) };
    }
    Ok(())
}

#[cfg(not(target_os = "android"))]
// reason: the Android version can fail.
#[allow(clippy::unnecessary_wraps)]
fn android_context(_env: &mut jni::Env<'_>, _context: &JObject<'_>) -> jni::errors::Result<()> {
    Ok(())
}

/// `Listener.wake()` from a runtime thread, attached to the VM once and kept attached.
fn waker(listener: Global<JObject<'static>>) -> app_sync::Wake {
    let listener = Arc::new(listener);
    Arc::new(move || {
        let Ok(vm) = JavaVM::singleton() else {
            return;
        };
        let called = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
            env.call_method(listener.as_ref(), jni_str!("wake"), jni_sig!(() -> ()), &[])?;
            Ok(())
        });
        if let Err(err) = called {
            tracing::warn!(error = %err, "Listener.wake failed");
        }
    })
}

#[unsafe(export_name = "Java_net_darii_dail_Native_open")]
pub extern "system" fn open<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    config: JString<'caller>,
    listener: JObject<'caller>,
    context: JObject<'caller>,
) -> JString<'caller> {
    env.with_env(|env| -> jni::errors::Result<_> {
        let config = config.try_to_string(env)?;
        android_context(env, &context)?;
        let listener = env.new_global_ref(&listener)?;
        let answer = calls::open(&config, waker(listener));
        JString::from_str(env, answer)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(export_name = "Java_net_darii_dail_Native_event")]
pub extern "system" fn event<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jint,
    event: JString<'caller>,
) -> JString<'caller> {
    env.with_env(|env| -> jni::errors::Result<_> {
        let event = event.try_to_string(env)?;
        JString::from_str(env, calls::event(handle, &event))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(export_name = "Java_net_darii_dail_Native_frame")]
pub extern "system" fn frame<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jint,
) -> JString<'caller> {
    env.with_env(|env| -> jni::errors::Result<_> { JString::from_str(env, calls::frame(handle)) })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(export_name = "Java_net_darii_dail_Native_blob")]
pub extern "system" fn blob<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jint,
    request: JString<'caller>,
) -> JByteArray<'caller> {
    env.with_env(|env| -> jni::errors::Result<_> {
        let request = request.try_to_string(env)?;
        match calls::blob(handle, &request) {
            Some(bytes) => env.byte_array_from_slice(&bytes),
            None => Ok(JByteArray::null()),
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(export_name = "Java_net_darii_dail_Native_background")]
pub extern "system" fn background<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jint,
    task: JString<'caller>,
) -> JString<'caller> {
    env.with_env(|env| -> jni::errors::Result<_> {
        let task = task.try_to_string(env)?;
        JString::from_str(env, calls::background(handle, &task))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(export_name = "Java_net_darii_dail_Native_close")]
pub extern "system" fn close<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jint,
) {
    env.with_env(|_env| -> jni::errors::Result<()> {
        calls::close(handle);
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}
