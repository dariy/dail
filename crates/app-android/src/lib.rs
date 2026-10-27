//! dail's Android host (ADR 0009, `ARCHITECTURE.md §7.5`), built as `libdail.so`.
//!
//! The session, the presenter and the six calls are `app-host`'s (ADR 0010). This crate adds what
//! is Android's: the JNI exports (`jni.rs`), keyring's Keystore store (`secrets`) and logcat
//! (`logging`), passed to the host as [`PLATFORM`].

mod jni;
pub mod logging;
pub mod secrets;

/// Android's credential store, logs and install kind, for `app_host::calls::open`.
pub const PLATFORM: app_host::Platform = app_host::Platform {
    secrets: secrets::store,
    logging: logging::init,
    install: Some(app_core::InstallKind::Android),
};
