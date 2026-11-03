//! dail's Android host (ADR 0009, `ARCHITECTURE.md §7.5`), built as `libdail.so`.
//!
//! The session, the presenter and the six calls are `app-host`'s (ADR 0010). This crate adds what
//! is Android's: the JNI exports (`jni.rs`), keyring's Keystore store (`secrets`) and logcat
//! (`logging`), passed to the host as [`PLATFORM`].

mod jni;
pub mod logging;
pub mod secrets;

/// Android's credential store and logs, for `app_host::calls::open`. No install kind: Android never checks for
/// updates (STORE-15, ADR 0011 amended). Google Play, F-Droid and Obtainium update the app, and Play bans updating
/// outside its own mechanism; without an install kind there is no check and no banner.
pub const PLATFORM: app_host::Platform = app_host::Platform {
    secrets: secrets::store,
    logging: logging::init,
    install: None,
};
