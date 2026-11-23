//! A device's link to its dail server (ADR 0017, SRV-5): [`RemoteBackend`], the link task and
//! [`RemoteSession`], the front ends' way to a remote profile (`Connection::Remote`).
//!
//! A remote profile's session runs its own engine in mirror mode over the local store
//! ([`crate::Session::open_remote`]). The engine emits only device-local effects
//! (`Effect::is_device_local`), which [`RemoteBackend`] hands to the local backend. A body the
//! local store does not have is read from the server (`blob`) and kept in the local store (its
//! content address checked), so it opens offline too.
//!
//! The link task dials the server whenever `state.link.server` is pinned (from `[remote]` at
//! start, or right after pairing): `hello` with the device token from the [`SecretStore`], then
//! it **replays** the offline queue (`link.queued`, persisted in the local store) before
//! anything else, so that the server has the device's offline work before its snapshot is
//! taken; then a snapshot (`state` → `remote_synced {snapshot: true}`), `watch` from that
//! revision (each `changed` ingested as `remote_synced`, a `resync` as a snapshot) and a replay
//! of what was queued meanwhile. A replay sends the entries in order, each with its
//! own request ID, seed and origin (an `attach_file` uploads its local file first). A delivered
//! entry leaves the queue (`queued_delivered`); one the server refuses (its engine's
//! rejection, or a request refusal such as an origin the device may not claim) moves to
//! `link.rejected` (`queued_rejected`, with the error and the time) and the replay goes on; a
//! lost connection stops it, and the rest waits for the next connection. Only once the queue
//! is empty does the link report `link_changed {online}`; until then a new command is queued by
//! the engine behind the others. The snapshot after the first replay takes the optimistic
//! changes of refused entries back (and a fresh one after a later replay with refusals). Then
//! the newest bodies are pre-loaded in the background
//! ([`RemoteOptions::preload_bodies`]). When dialing fails or the connection is lost the task
//! reports `link_failed` (offline, with the error) and dials again after a back-off that
//! doubles from [`RemoteOptions::backoff_min`] to [`RemoteOptions::backoff_max`] and starts
//! over once a connection came up.
//!
//! [`RemoteSession::submit`] routes each command by its placement (`Command::placement`):
//! - `client` and `client_with_blob`: the local session only;
//! - a query for the server: the server while connected (its receipt, with bodies), else the
//!   mirror;
//! - any other command for the server: applied locally first (optimistic), with a fresh seed.
//!   If the engine queued it (the link is not online, `OfflinePolicy::Queue`) or refused it
//!   offline (`ERR014`, `OfflinePolicy::Refuse`), that is the answer. Else it is submitted to the
//!   server with the same seed (so both create the same IDs) and a request ID derived from it,
//!   and the server's answer is the one returned. If the server refuses what the mirror
//!   accepted, the mirror is reset to a fresh snapshot, so the optimistic change does not stay;
//!   the caller gets the refusal. `attach_file` (`server_with_upload`) first uploads the file
//!   (`put_file`) and sends the server's path; the mirror keeps the local one. A command that
//!   finds older ones still queued, or whose answer is lost with the connection, is appended
//!   to the queue (`requeued`, same request ID: the server answers a known one from memory) and
//!   sent after them.
//!
//! An origin the device's role may not claim (from `hello`) is refused before anything is
//! applied (`ERR213`). A refusal by the server (`ERR213`, a JSON-RPC error) keeps the link up;
//! only a lost connection drops it and dials again.
//!
//! Without a pinned link (a plain profile, or a pairing in progress) everything runs locally.

use std::sync::Arc;
use std::time::Duration;

use std::collections::{HashSet, VecDeque};

use app_core::{
    AppState, BodyState, Command, CommandKind, Effect, Event, LinkStatus, MailboxRole,
    OfflinePolicy, Origin, PREFETCH_MAX_BYTES, Placement, QueuedCommand, RequestId, ServerEndpoint,
};
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use crate::backend::{Backend, BoxFuture};
use crate::clock::{Clock, SeedSource};
use crate::error::SyncError;
use crate::remote::{RemoteChange, RemoteClient, RemoteTarget};
use crate::secrets::SecretStore;
use crate::session::{PreviewResult, SessionClient, SubmitResult};

/// Timing of the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteOptions {
    /// The first pause before dialing again after a failure.
    pub backoff_min: Duration,
    /// The longest pause; the pause doubles after each failure up to this.
    pub backoff_max: Duration,
    /// How long [`RemoteSession::flush`] and [`RemoteSession::wait_settled`] wait for the link.
    pub settle_timeout: Duration,
    /// After each (re)connect, the bodies of this many of the newest messages (INBOX first,
    /// then the other mailboxes, each newest first; none larger than `PREFETCH_MAX_BYTES`) are
    /// pre-loaded into the local store in the background, those already there skipped, so
    /// they open offline. 0 turns pre-loading off; it pauses on a metered network.
    pub preload_bodies: usize,
    /// How many connections of their own the pre-loading uses at once (at least 1). They are
    /// not the commands' connection, so commands never wait for a body.
    pub preload_connections: usize,
}

impl Default for RemoteOptions {
    fn default() -> Self {
        Self {
            backoff_min: Duration::from_secs(1),
            backoff_max: Duration::from_secs(60),
            settle_timeout: Duration::from_secs(15),
            preload_bodies: 200,
            preload_connections: 2,
        }
    }
}

/// Requests from the submit side to the link task.
enum Control {
    /// Take a fresh snapshot (newer than or as new as the mirror) and ingest it, then answer.
    Sync(oneshot::Sender<()>),
    /// Drop the connection and dial again now.
    Reconnect,
}

/// The connection to the server, shared by the link task, the backend and the sessions.
pub struct RemoteLink {
    client: Mutex<Option<RemoteClient>>,
    control: mpsc::UnboundedSender<Control>,
    receiver: Mutex<Option<mpsc::UnboundedReceiver<Control>>>,
    options: RemoteOptions,
}

impl std::fmt::Debug for RemoteLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteLink")
            .field("online", &self.is_online())
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl RemoteLink {
    /// A link that is not connected yet.
    pub fn new(options: RemoteOptions) -> Arc<Self> {
        let (control, receiver) = mpsc::unbounded_channel();
        Arc::new(Self {
            client: Mutex::new(None),
            control,
            receiver: Mutex::new(Some(receiver)),
            options,
        })
    }

    /// The connection while online.
    pub fn client(&self) -> Option<RemoteClient> {
        self.client.lock().clone()
    }

    /// Whether commands for the server are forwarded now.
    pub fn is_online(&self) -> bool {
        self.client.lock().is_some()
    }

    fn set_client(&self, client: Option<RemoteClient>) {
        *self.client.lock() = client;
    }

    /// Drops the connection (the session is closing).
    pub(crate) fn close(&self) {
        self.set_client(None);
    }

    /// Starts the link task for `session` (once; later calls start nothing).
    /// `clock` dates the refusals of a replay; `store` is the local backend the pre-loaded
    /// bodies go to.
    pub(crate) fn spawn(
        self: &Arc<Self>,
        session: SessionClient,
        secrets: Arc<dyn SecretStore>,
        clock: Arc<dyn Clock>,
        store: Arc<dyn Backend>,
    ) -> Option<JoinHandle<()>> {
        let control = self.receiver.lock().take()?;
        Some(tokio::spawn(run(
            Arc::clone(self),
            control,
            Task {
                session,
                secrets,
                clock,
                store,
            },
        )))
    }

    /// Asks the link task for a fresh snapshot and waits until it is ingested (at most the
    /// settle timeout). Does nothing while offline.
    async fn sync(&self) {
        let (ack, done) = oneshot::channel();
        if self.control.send(Control::Sync(ack)).is_ok() {
            let _ = tokio::time::timeout(self.options.settle_timeout, done).await;
        }
    }

    fn reconnect(&self) {
        self.set_client(None);
        let _ = self.control.send(Control::Reconnect);
    }
}

/// The pinned server of `state`, without the transient offer: what the link dials.
fn pinned(state: &AppState) -> Option<ServerEndpoint> {
    let server = state.link.as_ref()?.server.as_ref()?;
    (server.is_pinned() && server.token.is_some()).then(|| ServerEndpoint {
        offered: None,
        pairing: None,
        ..server.clone()
    })
}

/// How a connected period ended.
enum Ended {
    /// The connection broke (or was dropped on purpose): dial again.
    Lost(String),
    /// The link was removed or now names another server.
    Unlinked,
    /// The session is gone.
    Gone,
}

/// How a pause before dialing again ended.
enum Pause {
    Retry,
    Unlinked,
    Gone,
}

/// What the link task works with.
struct Task {
    session: SessionClient,
    secrets: Arc<dyn SecretStore>,
    clock: Arc<dyn Clock>,
    store: Arc<dyn Backend>,
}

async fn run(link: Arc<RemoteLink>, mut control: mpsc::UnboundedReceiver<Control>, task: Task) {
    let Task {
        session, secrets, ..
    } = &task;
    let mut states = session.subscribe();
    let mut backoff = link.options.backoff_min;
    loop {
        let (server, status) = {
            let state = states.borrow_and_update();
            (pinned(&state), state.link.as_ref().map(|l| l.status))
        };
        let Some(server) = server else {
            // Not linked (a plain profile, or a pairing in progress): wait for a pin.
            tokio::select! {
                changed = states.changed() => if changed.is_err() { return },
                ctl = control.recv() => match ctl {
                    Some(Control::Sync(_) | Control::Reconnect) => {}
                    None => return,
                },
            }
            continue;
        };
        if status != Some(LinkStatus::Connecting)
            && session
                .ingest(Event::LinkChanged {
                    status: LinkStatus::Connecting,
                })
                .await
                .is_err()
        {
            return;
        }
        let error = match dial(&server, secrets).await {
            Ok(client) => {
                let ended = online(&link, &mut control, &task, &mut states, &server, client).await;
                link.set_client(None);
                match ended {
                    Ended::Lost(error) => {
                        backoff = link.options.backoff_min;
                        error
                    }
                    Ended::Unlinked => continue,
                    Ended::Gone => return,
                }
            }
            Err(err) => err.to_string(),
        };
        tracing::info!(host = %server.host, port = server.port, %error, "dail server link down");
        let still_pinned = pinned(&session.state()).as_ref() == Some(&server);
        if still_pinned && session.ingest(Event::LinkFailed { error }).await.is_err() {
            return;
        }
        match pause(backoff, &mut control, &mut states, &server).await {
            Pause::Retry => backoff = (backoff * 2).min(link.options.backoff_max),
            Pause::Unlinked => backoff = link.options.backoff_min,
            Pause::Gone => return,
        }
    }
}

/// Reads the device token and connects.
async fn dial(
    server: &ServerEndpoint,
    secrets: &Arc<dyn SecretStore>,
) -> Result<RemoteClient, SyncError> {
    let (Some(fingerprint), Some(token)) = (server.fingerprint.clone(), server.token.clone())
    else {
        return Err(SyncError::Remote("the server is not paired".to_owned()));
    };
    let store = Arc::clone(secrets);
    let token = tokio::task::spawn_blocking(move || store.get(&token))
        .await
        .map_err(|err| SyncError::Keyring(format!("keyring task failed: {err}")))??
        .ok_or_else(|| {
            SyncError::Remote(
                "no device token in the keyring; pair this profile again (`dail remote add`)"
                    .to_owned(),
            )
        })?;
    RemoteClient::connect(RemoteTarget {
        host: server.host.clone(),
        port: server.port,
        fingerprint,
        token,
    })
    .await
}

/// Ingests the snapshot, replays the queue, then ingests the changes of `client` (replaying
/// what gets queued meanwhile) until the connection ends.
async fn online(
    link: &RemoteLink,
    control: &mut mpsc::UnboundedReceiver<Control>,
    task: &Task,
    states: &mut watch::Receiver<Arc<AppState>>,
    server: &ServerEndpoint,
    client: RemoteClient,
) -> Ended {
    let session = &task.session;
    let gone = |_: SyncError| Ended::Gone;
    // Shared before the replay: queries go to the server, and a command submitted meanwhile is
    // queued by the engine (the link is not online yet) behind the others.
    link.set_client(Some(client.clone()));
    // The queue first, so that the server has what the device did offline before the snapshot
    // is taken: the snapshot then never takes it from the mirror (an offline draft, an open
    // composer). It does take back what the server refused.
    if let Err(ended) = replay(task, &client, false).await {
        return ended;
    }
    let (mut revision, snapshot) = match client.state().await {
        Ok(state) => state,
        Err(err) => return Ended::Lost(err.to_string()),
    };
    if let Err(err) = session.ingest(synced(revision, snapshot, true)).await {
        return gone(err);
    }
    let mut changes = match client.watch(Some(revision)).await {
        Ok(changes) => changes,
        Err(err) => return Ended::Lost(err.to_string()),
    };
    // What was queued since, then online once nothing waits.
    match replay(task, &client, true).await {
        Ok(refused) => {
            if refused && let Err(ended) = resync(&client, session, &mut revision).await {
                return ended;
            }
        }
        Err(ended) => return ended,
    }
    tracing::info!(
        host = %server.host,
        port = server.port,
        revision,
        "connected to the dail server"
    );
    let _preload = Preload(tokio::spawn(preload(
        Arc::clone(&task.store),
        client.clone(),
        session.state(),
        link.options,
    )));
    loop {
        tokio::select! {
            change = changes.next() => {
                let event = match change {
                    Some(RemoteChange::Changed { revision: next, diff }) => {
                        revision = revision.max(next);
                        Event::RemoteSynced { revision: next, diff, snapshot: false }
                    }
                    Some(RemoteChange::Resync { revision: next, state }) => {
                        revision = next;
                        Event::RemoteSynced { revision: next, diff: state, snapshot: true }
                    }
                    None => {
                        return Ended::Lost(format!(
                            "lost the connection to {}:{}",
                            server.host, server.port
                        ));
                    }
                };
                if let Err(err) = session.ingest(event).await {
                    return gone(err);
                }
            }
            ctl = control.recv() => match ctl {
                // The task reads no change while it waits for the snapshot, so the snapshot is
                // at least as new as every change it ingested.
                Some(Control::Sync(ack)) => {
                    if let Err(ended) = resync(&client, session, &mut revision).await {
                        return ended;
                    }
                    let _ = ack.send(());
                }
                Some(Control::Reconnect) => {
                    return Ended::Lost("the connection was dropped to dial again".to_owned());
                }
                None => return Ended::Gone,
            },
            changed = states.changed() => {
                if changed.is_err() {
                    return Ended::Gone;
                }
                let (still, waiting) = {
                    let state = states.borrow_and_update();
                    (
                        pinned(&state).as_ref() == Some(server),
                        state.link.as_ref().is_some_and(|link| !link.queued.is_empty()),
                    )
                };
                if !still {
                    tracing::info!(host = %server.host, port = server.port, "dail server link removed");
                    return Ended::Unlinked;
                }
                // Requeued while online (an answer lost, or older commands waiting).
                if waiting {
                    match replay(task, &client, false).await {
                        Ok(true) => {
                            if let Err(ended) = resync(&client, session, &mut revision).await {
                                return ended;
                            }
                        }
                        Ok(false) => {}
                        Err(ended) => return ended,
                    }
                }
            }
        }
    }
}

/// Takes a fresh snapshot (at least as new as `revision`) and ingests it, then waits until
/// the engine has it.
async fn resync(
    client: &RemoteClient,
    session: &SessionClient,
    revision: &mut u64,
) -> Result<(), Ended> {
    match client.state().await {
        Ok((next, snapshot)) if next >= *revision => {
            *revision = next;
            session
                .ingest(synced(next, snapshot, true))
                .await
                .map_err(|_| Ended::Gone)?;
        }
        Ok(_) => {}
        Err(err) => return Err(Ended::Lost(err.to_string())),
    }
    // Answered once the engine has it: inputs are taken in order.
    session.flush().await.map_err(|_| Ended::Gone)
}

/// The fate of one queued command on replay.
enum Delivery {
    /// The server ran it.
    Delivered,
    /// The server (or the upload of its file) refused it: code and message.
    Refused(String, String),
    /// The connection broke: it stays queued.
    Lost(String),
}

/// The `ERR…` code a message starts with (ADR 0013).
fn error_code(message: &str) -> String {
    message
        .split_once(':')
        .map(|(code, _)| code)
        .filter(|code| code.starts_with("ERR"))
        .unwrap_or("ERR211")
        .to_owned()
}

/// Sends one queued command with its own request ID, seed and origin.
async fn deliver(client: &RemoteClient, entry: &QueuedCommand) -> Delivery {
    let refused = |err: &SyncError| {
        let message = err.to_string();
        Delivery::Refused(error_code(&message), message)
    };
    if let Err(err) = claimable(client, entry.origin) {
        return refused(&err);
    }
    let command = match &entry.command {
        Command::AttachFile { draft, path, index } => {
            let bytes = match read_upload(path).await {
                Ok(bytes) => bytes,
                Err(err) => return refused(&err),
            };
            match client
                .put_file(app_core::draft::file_name(path), &bytes)
                .await
            {
                Ok(path) => Command::AttachFile {
                    draft: *draft,
                    path,
                    index: *index,
                },
                Err(err @ SyncError::Refused { .. }) => return refused(&err),
                Err(err) => return Delivery::Lost(err.to_string()),
            }
        }
        command => command.clone(),
    };
    match client
        .submit(&command, entry.origin, entry.request_id, entry.seed)
        .await
    {
        Ok(Ok(_)) => Delivery::Delivered,
        Ok(Err(rejected)) => Delivery::Refused(rejected.code().to_owned(), rejected.to_string()),
        Err(err @ SyncError::Refused { .. }) => refused(&err),
        Err(err) => Delivery::Lost(err.to_string()),
    }
}

/// Sends the queue in order until it is empty (what gets queued meanwhile included), each
/// outcome ingested (`queued_delivered`, `queued_rejected`). With `go_online`, reports
/// `link_changed {online}` once nothing waits. `Ok(true)` if the server refused any.
async fn replay(task: &Task, client: &RemoteClient, go_online: bool) -> Result<bool, Ended> {
    let session = &task.session;
    let mut sent: HashSet<RequestId> = HashSet::new();
    let mut refused = false;
    loop {
        // Every input so far is in the state, the outcomes of this replay included.
        session.flush().await.map_err(|_| Ended::Gone)?;
        let state = session.state();
        let Some(link) = state.link.as_ref() else {
            return Ok(refused);
        };
        let waiting: Vec<QueuedCommand> = link
            .queued
            .iter()
            .filter(|entry| !sent.contains(&entry.request_id))
            .cloned()
            .collect();
        if waiting.is_empty() {
            if go_online && !link.is_online() {
                session
                    .ingest(Event::LinkChanged {
                        status: LinkStatus::Online,
                    })
                    .await
                    .map_err(|_| Ended::Gone)?;
                // Once more: a command may have been queued just before.
                continue;
            }
            return Ok(refused);
        }
        for entry in waiting {
            let event = match deliver(client, &entry).await {
                Delivery::Delivered => Event::QueuedDelivered {
                    request_id: entry.request_id,
                },
                Delivery::Refused(code, error) => {
                    tracing::warn!(command = entry.command.name(), %error, "the dail server refused a queued command");
                    refused = true;
                    Event::QueuedRejected {
                        request_id: entry.request_id,
                        code,
                        error,
                        at_ms: task.clock.now_ms(),
                    }
                }
                Delivery::Lost(error) => return Err(Ended::Lost(error)),
            };
            sent.insert(entry.request_id);
            session.ingest(event).await.map_err(|_| Ended::Gone)?;
        }
    }
}

/// Aborts the pre-loading when the connection ends.
struct Preload(JoinHandle<()>);

impl Drop for Preload {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The bodies to pre-load: of the newest `limit` messages with a body on the server, INBOX
/// first, none larger than `PREFETCH_MAX_BYTES`.
fn preload_candidates(state: &AppState, limit: usize) -> Vec<String> {
    let in_inbox = |mailbox| {
        state
            .mailboxes
            .get(mailbox)
            .is_some_and(|m| m.role == Some(MailboxRole::Inbox))
    };
    let mut messages: Vec<(bool, i64, &str)> = state
        .messages
        .values()
        .filter(|m| m.size <= PREFETCH_MAX_BYTES)
        .filter_map(|m| match &m.body {
            BodyState::Fetched { blob } => Some((!in_inbox(&m.mailbox), -m.date_ms, blob.as_str())),
            BodyState::NotFetched | BodyState::Failed { .. } => None,
        })
        .collect();
    messages.sort_unstable();
    let mut seen = HashSet::new();
    messages
        .into_iter()
        .filter(|(_, _, blob)| seen.insert(*blob))
        .take(limit)
        .map(|(_, _, blob)| blob.to_owned())
        .collect()
}

/// Pre-loads the newest bodies of `state` into `store` over connections of their own (see
/// [`RemoteOptions::preload_bodies`]).
async fn preload(
    store: Arc<dyn Backend>,
    client: RemoteClient,
    state: Arc<AppState>,
    options: RemoteOptions,
) {
    if options.preload_bodies == 0 || state.status.metered {
        return;
    }
    let mut waiting = VecDeque::new();
    for blob in preload_candidates(&state, options.preload_bodies) {
        if !matches!(store.get_blob(&blob).await, Ok(Some(_))) {
            waiting.push_back(blob);
        }
    }
    if waiting.is_empty() {
        return;
    }
    let total = waiting.len();
    let waiting = Arc::new(Mutex::new(waiting));
    let mut workers = tokio::task::JoinSet::new();
    for _ in 0..options.preload_connections.clamp(1, total) {
        let (store, client, waiting) = (Arc::clone(&store), client.clone(), Arc::clone(&waiting));
        workers.spawn(async move {
            let connection = match client.fork().await {
                Ok(connection) => connection,
                Err(err) => {
                    tracing::debug!(error = %err, "no connection to pre-load bodies");
                    return;
                }
            };
            loop {
                let Some(blob) = waiting.lock().pop_front() else {
                    return;
                };
                match connection.blob(&blob).await {
                    Ok(Some(bytes)) => {
                        cache_blob(store.as_ref(), &blob, bytes).await;
                    }
                    Ok(None) => {}
                    Err(err) => {
                        tracing::debug!(error = %err, "pre-loading bodies stopped");
                        return;
                    }
                }
            }
        });
    }
    while workers.join_next().await.is_some() {}
    tracing::debug!(bodies = total, "bodies pre-loaded");
}

/// Keeps the server's copy of blob `id` in the local store if it is what its content address
/// says; returns it if so.
async fn cache_blob(store: &dyn Backend, id: &str, bytes: Vec<u8>) -> Option<Vec<u8>> {
    if app_io::blob_id(&bytes) != id {
        tracing::warn!(
            blob = id,
            "the server's body does not match its content address"
        );
        return None;
    }
    if let Err(err) = store.put_blob(bytes.clone()).await {
        tracing::warn!(blob = id, error = %err, "cannot keep the server's body");
    }
    Some(bytes)
}

fn synced(revision: u64, diff: app_core::StateDiff, snapshot: bool) -> Event {
    Event::RemoteSynced {
        revision,
        diff: Box::new(diff),
        snapshot,
    }
}

/// Waits `delay`, or less if asked to dial now, the link changes or the session ends.
async fn pause(
    delay: Duration,
    control: &mut mpsc::UnboundedReceiver<Control>,
    states: &mut watch::Receiver<Arc<AppState>>,
    server: &ServerEndpoint,
) -> Pause {
    let sleep = tokio::time::sleep(delay);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            () = &mut sleep => return Pause::Retry,
            ctl = control.recv() => match ctl {
                Some(Control::Reconnect) => return Pause::Retry,
                // Offline: there is nothing to sync with; the waiter is answered by the drop.
                Some(Control::Sync(_)) => {}
                None => return Pause::Gone,
            },
            changed = states.changed() => {
                if changed.is_err() {
                    return Pause::Gone;
                }
                if pinned(&states.borrow_and_update()).as_ref() != Some(server) {
                    return Pause::Unlinked;
                }
            }
        }
    }
}

/// The backend of a remote profile: the device-local effects run on `inner` (the local store,
/// files, updates, pairing), and bodies the local store lacks are read from the server.
pub struct RemoteBackend {
    inner: Arc<dyn Backend>,
    link: Arc<RemoteLink>,
}

impl RemoteBackend {
    /// `inner` for the device's own effects, the server through `link`.
    pub fn new(inner: Arc<dyn Backend>, link: Arc<RemoteLink>) -> Self {
        Self { inner, link }
    }

    /// The server's copy of blob `id`, while connected, kept in the local store.
    async fn server_blob(&self, id: &str) -> Result<Option<Vec<u8>>, SyncError> {
        let Some(client) = self.link.client() else {
            return Ok(None);
        };
        Ok(match client.blob(id).await? {
            Some(bytes) => cache_blob(self.inner.as_ref(), id, bytes).await,
            None => None,
        })
    }
}

impl Backend for RemoteBackend {
    fn name(&self) -> &'static str {
        "remote"
    }

    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>> {
        self.inner.load_state()
    }

    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>> {
        Box::pin(async move {
            // `save_attachments` (`client_with_blob`): the body comes from the server when the
            // device does not have it.
            if let Effect::SaveAttachments {
                message,
                blob,
                parts,
                dir,
            } = &effect
                && self.inner.get_blob(blob).await?.is_none()
            {
                let raw = self.server_blob(blob).await?;
                return Ok(vec![
                    crate::saves::save_attachments(*message, raw, parts, dir).await,
                ]);
            }
            self.inner.execute(effect).await
        })
    }

    fn get_blob<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Option<Vec<u8>>, SyncError>> {
        Box::pin(async move {
            match self.inner.get_blob(id).await? {
                Some(bytes) => Ok(Some(bytes)),
                None => self.server_blob(id).await,
            }
        })
    }

    fn put_blob(&self, bytes: Vec<u8>) -> BoxFuture<'_, Result<Option<String>, SyncError>> {
        self.inner.put_blob(bytes)
    }
}

/// A remote profile's session as the front ends reach it (`Connection::Remote`): the local
/// session, with the commands for the server forwarded over the link (see the module doc).
#[derive(Clone)]
pub struct RemoteSession {
    local: SessionClient,
    link: Arc<RemoteLink>,
    seeds: Arc<dyn SeedSource>,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for RemoteSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSession")
            .field("local", &self.local)
            .field("link", &self.link)
            .finish_non_exhaustive()
    }
}

/// The request ID of a submission dispatched with `seed`, as the engine derives it for a queued
/// command.
fn request_id(seed: u128) -> RequestId {
    RequestId::from_uuid(uuid::Uuid::from_u128(seed))
}

impl RemoteSession {
    pub(crate) fn new(
        local: SessionClient,
        link: Arc<RemoteLink>,
        seeds: Arc<dyn SeedSource>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            local,
            link,
            seeds,
            clock,
        }
    }

    /// The local session (the mirror).
    pub const fn local(&self) -> &SessionClient {
        &self.local
    }

    /// The link to the server.
    pub const fn link(&self) -> &Arc<RemoteLink> {
        &self.link
    }

    /// Dials the server now instead of waiting out the back-off, when the link is down (a
    /// phone's network came back). Does nothing while it connects or is connected, or without
    /// a pinned server.
    pub fn dial_now(&self) {
        let offline = self
            .local
            .state()
            .link
            .as_ref()
            .is_some_and(|link| link.status == LinkStatus::Offline);
        if offline && self.linked() && !self.link.is_online() {
            self.link.reconnect();
        }
    }

    /// Whether the profile is linked to a server it can connect to (a pinned server).
    fn linked(&self) -> bool {
        pinned(&self.local.state()).is_some()
    }

    /// Dispatches a command where its placement says (see the module doc). The outer error is
    /// the runtime's (the session is gone, the link broke: `ERR211`, the server refused the
    /// request: `ERR213`); the inner one the engine's rejection, the server's for commands that
    /// run there, or the mirror's (`ERR014` offline).
    pub async fn submit(&self, cmd: Command, origin: Origin) -> Result<SubmitResult, SyncError> {
        let placement = cmd.placement();
        if !placement.on_server() || !self.linked() {
            return self.local.submit(cmd, origin).await;
        }
        let client = self.link.client();
        if cmd.kind() == CommandKind::Query {
            return match client {
                Some(client) => {
                    claimable(&client, origin)?;
                    let seed = self.seeds.next_seed();
                    let answer = client.submit(&cmd, origin, request_id(seed), seed).await;
                    if let Err(err) = &answer
                        && is_lost(err)
                    {
                        self.link.reconnect();
                    }
                    answer
                }
                // Answered from the mirror.
                None => self.local.submit(cmd, origin).await,
            };
        }
        // Refused before anything is applied: the server would refuse it too.
        if let Some(client) = &client {
            claimable(client, origin)?;
        }
        // Uploaded first while connected; queued with the local path otherwise, uploaded on
        // replay.
        let uploaded = match (&cmd, placement, &client) {
            (
                Command::AttachFile { draft, path, index },
                Placement::ServerWithUpload,
                Some(client),
            ) if self.queue_is_empty() => {
                let bytes = read_upload(path).await?;
                let path = match client
                    .put_file(app_core::draft::file_name(path), &bytes)
                    .await
                {
                    Ok(path) => path,
                    Err(err) => {
                        if is_lost(&err) {
                            self.link.reconnect();
                        }
                        return Err(err);
                    }
                };
                Some(Command::AttachFile {
                    draft: *draft,
                    path,
                    index: *index,
                })
            }
            _ => None,
        };
        let seed = self.seeds.next_seed();
        let id = request_id(seed);
        let optimistic = self.local.submit_seeded(cmd.clone(), origin, seed).await?;
        let state = self.local.state();
        let link = state.link.as_ref();
        let queued = link.is_some_and(|link| link.queued.iter().any(|q| q.request_id == id));
        let refused_offline = matches!(optimistic, Err(app_core::CoreError::RefusedOffline { .. }));
        if queued || refused_offline {
            // Queued by the engine, to be replayed in order; or refused while offline.
            return Ok(optimistic);
        }
        let queueable = optimistic.is_ok() && cmd.offline_policy() == OfflinePolicy::Queue;
        let waiting = link.is_some_and(|link| !link.queued.is_empty());
        let client = match client {
            Some(client) if !(waiting && queueable) => client,
            // Older commands still wait (they go first), or the connection is gone since.
            _ if queueable => {
                self.requeue(id, seed, origin, cmd).await?;
                return Ok(optimistic);
            }
            Some(client) => client,
            // Rejected by the mirror, with nothing to ask instead.
            None if optimistic.is_err() => return Ok(optimistic),
            None => {
                return Err(SyncError::NotOnline {
                    command: cmd.name(),
                    status: status_name(link.map_or(LinkStatus::Offline, |link| link.status)),
                });
            }
        };
        let to_server = uploaded.unwrap_or_else(|| cmd.clone());
        match client.submit(&to_server, origin, id, seed).await {
            Ok(Ok(receipt)) => Ok(Ok(receipt)),
            Ok(Err(refused)) => {
                tracing::warn!(
                    command = to_server.name(),
                    error = %refused,
                    "the dail server refused a command"
                );
                // The mirror applied it: a fresh snapshot takes it back.
                if optimistic.is_ok() {
                    self.link.sync().await;
                }
                Ok(Err(refused))
            }
            // The server refused the request itself: the connection is fine, the mirror is
            // reset like for a rejected command.
            Err(err @ SyncError::Refused { .. }) => {
                tracing::warn!(command = to_server.name(), error = %err, "the dail server refused a request");
                if optimistic.is_ok() {
                    self.link.sync().await;
                }
                Err(err)
            }
            Err(err) => {
                self.link.reconnect();
                // Whether the server ran it is unknown: sent again on reconnect with the same
                // request ID, so that it runs once.
                if queueable {
                    self.requeue(id, seed, origin, cmd).await?;
                    return Ok(optimistic);
                }
                Err(err)
            }
        }
    }

    /// Whether nothing waits in the offline queue.
    fn queue_is_empty(&self) -> bool {
        self.local
            .state()
            .link
            .as_ref()
            .is_none_or(|link| link.queued.is_empty())
    }

    /// Appends a command the mirror applied to the offline queue (`requeued`), and waits until
    /// the engine has it.
    async fn requeue(
        &self,
        request_id: RequestId,
        seed: u128,
        origin: Origin,
        command: Command,
    ) -> Result<(), SyncError> {
        self.local
            .ingest(Event::Requeued {
                entry: Box::new(QueuedCommand {
                    request_id,
                    seed,
                    origin,
                    command,
                    queued_at_ms: self.clock.now_ms(),
                }),
            })
            .await?;
        self.local.flush().await
    }

    /// A dry run: on the server for its commands while online, else on the mirror.
    pub async fn preview(&self, cmd: Command, origin: Origin) -> Result<PreviewResult, SyncError> {
        if cmd.placement().on_server()
            && self.linked()
            && let Some(client) = self.link.client()
        {
            claimable(&client, origin)?;
            let answer = client.preview(&cmd, origin).await;
            if let Err(err) = &answer
                && is_lost(err)
            {
                self.link.reconnect();
            }
            return answer;
        }
        self.local.preview(cmd, origin).await
    }

    /// Waits until the local effects issued so far are done and, while online, the offline
    /// queue is replayed, the server's effects are done too and the mirror has the server's
    /// state after them.
    pub async fn flush(&self) -> Result<(), SyncError> {
        self.local.flush().await?;
        if !self.linked() {
            return Ok(());
        }
        let Some(client) = self.link.client() else {
            return Ok(());
        };
        self.wait_replayed().await;
        if let Err(err) = client.flush().await {
            if is_lost(&err) {
                self.link.reconnect();
            }
            return Err(err);
        }
        self.link.sync().await;
        self.local.flush().await
    }

    /// Waits (at most the settle timeout) until the offline queue is empty or the link is
    /// down.
    async fn wait_replayed(&self) {
        let mut states = self.local.subscribe();
        let wait = async {
            loop {
                let done = {
                    let state = states.borrow_and_update();
                    state.link.as_ref().is_none_or(|link| {
                        link.queued.is_empty() || link.status == LinkStatus::Offline
                    })
                };
                if done || states.changed().await.is_err() {
                    return;
                }
            }
        };
        let _ = tokio::time::timeout(self.link.options.settle_timeout, wait).await;
    }

    /// The mirror's state.
    pub fn state(&self) -> Arc<AppState> {
        self.local.state()
    }

    /// The mirror's state, kept current.
    pub fn subscribe(&self) -> watch::Receiver<Arc<AppState>> {
        self.local.subscribe()
    }

    /// Waits (at most the settle timeout) until a pinned link has connected or failed once, so
    /// that a one-shot run (`dail exec`) does not refuse its first command while the link is
    /// still dialing. Returns the link's status; `None` without a pinned link.
    pub async fn wait_settled(&self) -> Option<LinkStatus> {
        let mut states = self.local.subscribe();
        let wait = async {
            loop {
                let status = {
                    let state = states.borrow_and_update();
                    pinned(&state)?;
                    state.link.as_ref().map(|link| link.status)
                };
                // `online` in the state precedes the shared client by a moment.
                let settled = match status {
                    Some(LinkStatus::Online) => self.link.is_online(),
                    Some(LinkStatus::Offline) => true,
                    _ => false,
                };
                if settled {
                    return status;
                }
                tokio::select! {
                    changed = states.changed() => if changed.is_err() { return status },
                    () = tokio::time::sleep(Duration::from_millis(20)) => {}
                }
            }
        };
        match tokio::time::timeout(self.link.options.settle_timeout, wait).await {
            Ok(status) => status,
            Err(_) => self.local.state().link.as_ref().map(|link| link.status),
        }
    }
}

/// Whether `err` means the connection is gone (not a refusal by a working server).
const fn is_lost(err: &SyncError) -> bool {
    !matches!(err, SyncError::Refused { .. })
}

/// Refuses an origin the device's role may never claim on its server (`hello` names the
/// role), with the message the server would give.
fn claimable(client: &RemoteClient, origin: Origin) -> Result<(), SyncError> {
    let role = client.hello().device.role;
    // `mcp_allow_send` depends on the server's `--allow-send`: the server decides.
    if crate::serve::origin_allowed(role, origin, true) {
        return Ok(());
    }
    let origin = serde_json::to_value(origin)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    Err(SyncError::Refused {
        method: "submit".to_owned(),
        code: crate::serve::ORIGIN_REFUSED,
        message: format!(
            "this device is paired as `{}` and may not submit as `{origin}` (user devices: gui, tui, cli, batch; agent devices: mcp)",
            role.as_str()
        ),
    })
}

const fn status_name(status: LinkStatus) -> &'static str {
    match status {
        LinkStatus::Connecting => "connecting",
        LinkStatus::Online => "online",
        LinkStatus::Offline => "offline",
    }
}

/// Reads the file at `path` for `attach_file` (a regular file of at most `MAX_UPLOAD_BYTES`).
async fn read_upload(path: &str) -> Result<Vec<u8>, SyncError> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|err| SyncError::Remote(format!("cannot read {path}: {err}")))?;
    let limit = u64::try_from(crate::serve::MAX_UPLOAD_BYTES).unwrap_or(u64::MAX);
    if !meta.is_file() || meta.len() > limit {
        return Err(SyncError::Remote(format!(
            "{path} is not a regular file of at most {} MiB",
            crate::serve::MAX_UPLOAD_BYTES / (1024 * 1024)
        )));
    }
    tokio::fs::read(path)
        .await
        .map_err(|err| SyncError::Remote(format!("cannot read {path}: {err}")))
}
