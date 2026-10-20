//! `Connection`: a front-end's way to the engine, either its own session or the session of the
//! process that owns the profile (attached over IPC, [`crate::ipc`]).

use std::sync::Arc;

use app_core::{AppState, Command, Origin};
use tokio::sync::watch;

use crate::error::SyncError;
use crate::ipc::IpcClient;
use crate::session::{SessionClient, SubmitResult};

/// Where a front-end's commands go.
#[derive(Debug, Clone)]
pub enum Connection {
    /// A session in this process.
    Local(SessionClient),
    /// The session of another dail process (the GUI or `dail mcp`).
    Attached(IpcClient),
}

impl Connection {
    /// Dispatches a command. The outer error means the session is gone (or the IPC link broke);
    /// the inner one is the engine's rejection.
    pub async fn submit(&self, cmd: Command, origin: Origin) -> Result<SubmitResult, SyncError> {
        match self {
            Self::Local(client) => client.submit(cmd, origin).await,
            Self::Attached(client) => client.submit(&cmd, origin).await,
        }
    }

    /// A dry run of `cmd` (see [`crate::SessionClient::preview`]).
    pub async fn preview(
        &self,
        cmd: Command,
        origin: Origin,
    ) -> Result<crate::session::PreviewResult, SyncError> {
        match self {
            Self::Local(client) => client.preview(cmd, origin).await,
            Self::Attached(client) => client.preview(&cmd, origin).await,
        }
    }

    /// Waits until every effect issued so far has completed and been ingested.
    pub async fn flush(&self) -> Result<(), SyncError> {
        match self {
            Self::Local(client) => client.flush().await,
            Self::Attached(client) => client.flush().await,
        }
    }

    /// The current state.
    pub async fn state(&self) -> Result<Arc<AppState>, SyncError> {
        match self {
            Self::Local(client) => Ok(client.state()),
            Self::Attached(client) => client.state().await.map(Arc::new),
        }
    }

    /// The state, kept current: the local session's channel, or for an attached client one that
    /// is refreshed after each `changed` notification of the owner ([`IpcClient::subscribe`]).
    /// Either way the receiver's `changed()` fails once the session is gone.
    pub async fn subscribe(&self) -> Result<watch::Receiver<Arc<AppState>>, SyncError> {
        match self {
            Self::Local(client) => Ok(client.subscribe()),
            Self::Attached(client) => client.subscribe().await,
        }
    }

    /// Whether commands go to another process.
    pub const fn is_attached(&self) -> bool {
        matches!(self, Self::Attached(_))
    }
}
