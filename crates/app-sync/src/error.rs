//! `SyncError`: failures of the runtime (not of the engine; those are `CoreError`).

/// Runtime failure.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// The local store failed.
    #[error(transparent)]
    Store(#[from] app_io::IoError),
    /// The backend could not perform an effect.
    #[error("ERR201: {effect} failed: {reason}")]
    Effect {
        /// Name of the effect.
        effect: &'static str,
        /// Diagnostic.
        reason: String,
    },
    /// The backend does not implement this effect (yet).
    #[error("ERR202: backend `{backend}` does not support {effect}")]
    Unsupported {
        /// Name of the backend.
        backend: &'static str,
        /// Name of the effect.
        effect: &'static str,
    },
    /// The OS keyring failed.
    #[error("ERR203: keyring: {0}")]
    Keyring(String),
    /// TLS could not be configured.
    #[error("ERR204: TLS setup failed: {0}")]
    Tls(String),
    /// The local socket or the profile lock failed.
    #[error("ERR205: IPC: {0}")]
    Ipc(String),
    /// Another dail process holds the profile but does not accept connections.
    #[error(
        "ERR206: the profile {0} is in use by another dail process that does not accept connections; try again when it has finished"
    )]
    ProfileBusy(String),
    /// The session's engine task has stopped.
    #[error("ERR207: session closed")]
    SessionClosed,
}
