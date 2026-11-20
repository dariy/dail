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
    /// The dail server could not start or serve (its key, certificate, listener, or paired
    /// devices; ADR 0017).
    #[error("ERR210: dail server: {0}")]
    Serve(String),
    /// Talking to a dail server failed: it could not be reached, its certificate is not the
    /// pinned one, or it refused or did not follow the protocol (ADR 0017).
    #[error("ERR211: dail server link: {0}")]
    Remote(String),
    /// A command that runs on the dail server was submitted on a remote profile while the
    /// server is not connected. Remote profiles queue nothing yet (ADR 0017, SRV-5a).
    #[error(
        "ERR212: `{command}` runs on the dail server, which is not connected ({status}); nothing was changed, try again when the link is online"
    )]
    NotOnline {
        /// The command's name.
        command: &'static str,
        /// The link's status.
        status: &'static str,
    },
    /// The dail server answered a request of this device with a refusal (a JSON-RPC error, such
    /// as an origin its role may not claim). The link stays up (ADR 0017).
    #[error("ERR213: the dail server refused `{method}`: {message}")]
    Refused {
        /// The request (`submit`, `preview`, …).
        method: String,
        /// The JSON-RPC error code.
        code: i64,
        /// The server's message.
        message: String,
    },
}
