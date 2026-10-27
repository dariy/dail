//! dail's iOS host (ADR 0010, `ARCHITECTURE.md §7.6`), built as `libdail.a`.
//!
//! The session, the presenter and the six calls are `app-host`'s. This crate adds what is iOS's:
//! the C exports Swift calls (`ffi.rs`, declared in `include/dail.h`), the Keychain store
//! (`secrets`) and the log file (`logging`), passed to the host as [`PLATFORM`]. It builds and is
//! tested on every host; the iOS-only parts sit behind `cfg(target_os = "ios")`.

pub mod ffi;
pub mod logging;
pub mod secrets;

/// iOS's credential store, logs and install kind, for `app_host::calls::open`.
pub const PLATFORM: app_host::Platform = app_host::Platform {
    secrets: secrets::store,
    logging: logging::init,
    install: Some(app_core::InstallKind::Ios),
};
