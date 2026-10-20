//! `Bridge`: a front-end's connection to a session, for a UI thread that never awaits.
//!
//! The UI hands its commands to [`Bridge::submit`], each with a tag of its own choosing (why it
//! submitted it). A task on the tokio runtime submits them in order with the front-end's origin
//! and sends each result back with its tag, to be picked up by the UI ([`Bridge::replies`]).
//! Every state change and every reply calls the front-end's wake-up callback. The bridge runs
//! over a [`Connection`]: a session this process owns ([`Bridge::own`], shut down when the bridge
//! closes) or another process's session ([`Bridge::attach`], left running).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc as std_mpsc};

use app_core::{AppState, Command, CommandName, Origin};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::connection::Connection;
use crate::error::SyncError;
use crate::session::{Session, SubmitResult};

/// Wakes the front-end (e.g. `egui::Context::request_repaint`). Called from runtime threads.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// The result of one submitted command.
#[derive(Debug)]
pub struct Reply<T> {
    /// The tag it was submitted with.
    pub tag: T,
    /// Which command it was.
    pub name: CommandName,
    /// The engine's receipt or rejection, or the runtime failure.
    pub result: Result<SubmitResult, SyncError>,
}

/// The UI side of the connection.
pub struct Bridge<T> {
    requests: Option<mpsc::UnboundedSender<(T, Command)>>,
    replies: std_mpsc::Receiver<Reply<T>>,
    state: watch::Receiver<Arc<AppState>>,
    gone: Arc<AtomicBool>,
}

impl<T> std::fmt::Debug for Bridge<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bridge")
            .field("closed", &self.requests.is_none())
            .field("gone", &self.is_gone())
            .finish_non_exhaustive()
    }
}

impl<T: Send + 'static> Bridge<T> {
    /// Moves `session` onto a task of `runtime`. The task ends (and shuts the session down,
    /// saving unsaved drafts) once the bridge is closed or dropped; await the returned handle to
    /// wait for that.
    pub fn own(
        runtime: &Handle,
        session: Session,
        origin: Origin,
        wake: Wake,
    ) -> (Self, JoinHandle<()>) {
        let conn = Connection::Local(session.client());
        let state = session.subscribe();
        Self::spawn(runtime, conn, state, Some(session), origin, wake)
    }

    /// Runs over `conn` without owning its session (attached to another process, or a client of
    /// a session someone else shuts down). The task ends when the bridge is closed or dropped.
    pub async fn attach(
        conn: Connection,
        origin: Origin,
        wake: Wake,
    ) -> Result<(Self, JoinHandle<()>), SyncError> {
        let state = conn.subscribe().await?;
        Ok(Self::spawn(
            &Handle::current(),
            conn,
            state,
            None,
            origin,
            wake,
        ))
    }

    fn spawn(
        runtime: &Handle,
        conn: Connection,
        state: watch::Receiver<Arc<AppState>>,
        owner: Option<Session>,
        origin: Origin,
        wake: Wake,
    ) -> (Self, JoinHandle<()>) {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (reply_tx, reply_rx) = std_mpsc::channel();
        let gone = Arc::new(AtomicBool::new(false));
        let mut changes = state.clone();
        let (watch_wake, watch_gone) = (Arc::clone(&wake), Arc::clone(&gone));
        let watcher = runtime.spawn(async move {
            while changes.changed().await.is_ok() {
                watch_wake();
            }
            // The session is gone (shut down, or the owner process went away).
            watch_gone.store(true, Ordering::SeqCst);
            watch_wake();
        });
        let serve_gone = Arc::clone(&gone);
        let task = runtime.spawn(async move {
            serve(conn, origin, request_rx, reply_tx, &wake, &serve_gone).await;
            watcher.abort();
            if let Some(session) = owner
                && let Err(err) = session.shutdown().await
            {
                tracing::warn!(error = %err, "session shutdown failed");
            }
        });
        (
            Self {
                requests: Some(request_tx),
                replies: reply_rx,
                state,
                gone,
            },
            task,
        )
    }
}

impl<T> Bridge<T> {
    /// The current state. Cheap: a shared snapshot, unchanged until the next call.
    pub fn state(&self) -> Arc<AppState> {
        Arc::clone(&self.state.borrow())
    }

    /// Queues a command for the session. Ignored once the bridge is closed.
    pub fn submit(&self, tag: T, cmd: Command) {
        if let Some(requests) = &self.requests
            && requests.send((tag, cmd)).is_err()
        {
            tracing::warn!("session is gone; request dropped");
        }
    }

    /// Results that arrived since the last call, in submission order.
    pub fn replies(&self) -> Vec<Reply<T>> {
        self.replies.try_iter().collect()
    }

    /// Whether the session went away (shut down, or the owner process ended): no more changes
    /// will arrive and submits fail.
    pub fn is_gone(&self) -> bool {
        self.gone.load(Ordering::SeqCst)
    }

    /// Stops accepting commands; an owned session shuts down after the queued ones.
    pub fn close(&mut self) {
        self.requests = None;
    }
}

async fn serve<T>(
    conn: Connection,
    origin: Origin,
    mut requests: mpsc::UnboundedReceiver<(T, Command)>,
    replies: std_mpsc::Sender<Reply<T>>,
    wake: &Wake,
    gone: &AtomicBool,
) {
    while let Some((tag, cmd)) = requests.recv().await {
        let name = cmd.command_name();
        let result = conn.submit(cmd, origin).await;
        let closed = matches!(result, Err(SyncError::SessionClosed));
        if closed {
            gone.store(true, Ordering::SeqCst);
        }
        if replies.send(Reply { tag, name, result }).is_err() {
            break;
        }
        wake();
        if closed {
            return;
        }
    }
}
