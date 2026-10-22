//! Why a host call failed.

/// A failure of a host call; Kotlin receives it as `{"error": "<message>"}`.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// The JSON from Kotlin did not parse.
    #[error("invalid request: {0}")]
    Json(#[from] serde_json::Error),
    /// The tokio runtime did not start.
    #[error("runtime: {0}")]
    Runtime(#[from] std::io::Error),
    /// The profile did not open.
    #[error("profile: {0}")]
    Profile(#[from] app_io::IoError),
    /// The session failed (opening it, or a request to it).
    #[error("session: {0}")]
    Sync(#[from] app_sync::SyncError),
    /// Another host holds the profile.
    #[error("the profile {0} is in use")]
    Locked(String),
    /// The backend asked for is not in this build.
    #[error("the {0} backend is not in this build")]
    NoBackend(&'static str),
    /// The demo fixture did not load.
    #[error("demo: {0}")]
    Demo(String),
    /// The session is closed.
    #[error("the session is closed")]
    Closed,
}
