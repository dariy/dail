//! What a platform host gives the session: its credential store and its logs (ADR 0010).
//! `app-android` passes the Keystore store and logcat; [`DESKTOP`] is the OS keyring with no
//! logs, so that the host runs in desktop tests.

use std::path::Path;
use std::sync::Arc;

use app_sync::SecretStore;

use crate::error::HostError;

/// The platform parts of a host, as plain functions (no state, so they cross no FFI).
#[derive(Clone, Copy)]
pub struct Platform {
    /// The credential store of this platform, made when a live profile opens.
    pub secrets: fn() -> Result<Arc<dyn SecretStore>, HostError>,
    /// Starts the process's logs for the profile at this path. Called on every `open`; it must
    /// do its work once.
    pub logging: fn(&Path),
}

/// The desktop: the OS keyring (`KeyringSecrets`), and no logs.
pub const DESKTOP: Platform = Platform {
    secrets: keyring,
    logging: no_logs,
};

fn keyring() -> Result<Arc<dyn SecretStore>, HostError> {
    Ok(Arc::new(app_sync::KeyringSecrets))
}

fn no_logs(_profile_dir: &Path) {}
