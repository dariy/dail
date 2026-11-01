//! Why a host call failed.

/// A failure of a host call; the front-end (Kotlin, Swift) receives it as `{"error": "<message>"}`.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// The JSON from Kotlin did not parse.
    #[error("ERR301: invalid request: {0}")]
    Json(#[from] serde_json::Error),
    /// The tokio runtime did not start.
    #[error("ERR302: runtime: {0}")]
    Runtime(#[from] std::io::Error),
    /// The profile did not open.
    #[error(transparent)]
    Profile(#[from] app_io::IoError),
    /// The session failed (opening it, or a request to it).
    #[error(transparent)]
    Sync(#[from] app_sync::SyncError),
    /// Another host holds the profile.
    #[error("ERR303: the profile {0} is in use")]
    Locked(String),
    /// The backend asked for is not in this build.
    #[error("ERR304: the {0} backend is not in this build")]
    NoBackend(&'static str),
    /// The demo fixture did not load.
    #[error("ERR305: demo: {0}")]
    Demo(String),
    /// The credential store did not open.
    #[error("ERR306: secrets: {0}")]
    Secrets(String),
    /// No open host has this handle.
    #[error("ERR307: no host with handle {0}")]
    NoHost(i32),
    /// A debug task in a release build, or from an origin the bridge does not speak for.
    #[error("ERR308: debug bridge: {0}")]
    Debug(&'static str),
    /// A picked or shared file was not copied.
    #[error(transparent)]
    Import(#[from] crate::outgoing::ImportError),
    /// The session is closed.
    #[error("ERR309: the session is closed")]
    Closed,
}
