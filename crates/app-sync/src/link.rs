//! A device's link to its dail server (ADR 0017, SRV-5a): [`RemoteBackend`], the link task and
//! [`RemoteSession`], the front ends' way to a remote profile (`Connection::Remote`).
//!
//! A remote profile's session runs its own engine in mirror mode over the local store
//! ([`crate::Session::open_remote`]). The engine emits only device-local effects
//! (`Effect::is_device_local`), which [`RemoteBackend`] hands to the local backend; a body the
//! local store does not have is read from the server (`blob`).
//!
//! The link task dials the server whenever `state.link.server` is pinned (from `[remote]` at
//! start, or right after pairing): `hello` with the device token from the [`SecretStore`], a
//! snapshot (`state` → `remote_synced {snapshot: true}`), then `watch` from that revision, each
//! `changed` ingested as `remote_synced` and a `resync` as a snapshot. It reports
//! `link_changed {connecting|online}` and, when dialing fails or the connection is lost,
//! `link_failed` (offline, with the error), then dials again after a back-off that doubles from
//! [`RemoteOptions::backoff_min`] to [`RemoteOptions::backoff_max`] and starts over once a
//! connection came up.
//!
//! [`RemoteSession::submit`] routes each command by its placement (`Command::placement`):
//! - `client` and `client_with_blob`: the local session only;
//! - a query for the server: the server while online (its receipt, with bodies), else the
//!   mirror;
//! - any other command for the server while online: applied locally first (optimistic), with a
//!   fresh seed, then submitted to the server with the same seed (so both create the same IDs)
//!   and a request ID derived from it. The server's answer is the one returned. If the server
//!   refuses what the mirror accepted, the mirror is reset to a fresh snapshot, so the
//!   optimistic change does not stay; the caller gets the refusal. `attach_file`
//!   (`server_with_upload`) first uploads the file (`put_file`) and sends the server's path; the
//!   mirror keeps the local one;
//! - while not online (5a has no offline queue in the runtime): `ERR212`, and nothing is applied.
//!
//! An origin the device's role may not claim (from `hello`) is refused before anything is
//! applied (`ERR213`). A refusal by the server (`ERR213`, a JSON-RPC error) keeps the link up;
//! only a lost connection drops it and dials again.
//!
//! Without a pinned link (a plain profile, or a pairing in progress) everything runs locally.

use std::sync::Arc;
use std::time::Duration;

use app_core::{
    AppState, Command, CommandKind, Effect, Event, LinkStatus, Origin, Placement, RequestId,
    ServerEndpoint,
};
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use crate::backend::{Backend, BoxFuture};
use crate::clock::SeedSource;
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
}

impl Default for RemoteOptions {
    fn default() -> Self {
        Self {
            backoff_min: Duration::from_secs(1),
            backoff_max: Duration::from_secs(60),
            settle_timeout: Duration::from_secs(15),
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
    pub(crate) fn spawn(
        self: &Arc<Self>,
        session: SessionClient,
        secrets: Arc<dyn SecretStore>,
    ) -> Option<JoinHandle<()>> {
        let control = self.receiver.lock().take()?;
        Some(tokio::spawn(run(
            Arc::clone(self),
            control,
            session,
            secrets,
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

async fn run(
    link: Arc<RemoteLink>,
    mut control: mpsc::UnboundedReceiver<Control>,
    session: SessionClient,
    secrets: Arc<dyn SecretStore>,
) {
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
        let error = match dial(&server, &secrets).await {
            Ok(client) => {
                let ended =
                    online(&link, &mut control, &session, &mut states, &server, client).await;
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

/// Ingests the snapshot and the changes of `client` until the connection ends.
async fn online(
    link: &RemoteLink,
    control: &mut mpsc::UnboundedReceiver<Control>,
    session: &SessionClient,
    states: &mut watch::Receiver<Arc<AppState>>,
    server: &ServerEndpoint,
    client: RemoteClient,
) -> Ended {
    let gone = |_: SyncError| Ended::Gone;
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
    // Online before the client is shared: a command forwarded from now on reaches the engine
    // after `link_changed`, so the engine does not queue it.
    if let Err(err) = session
        .ingest(Event::LinkChanged {
            status: LinkStatus::Online,
        })
        .await
    {
        return gone(err);
    }
    link.set_client(Some(client.clone()));
    tracing::info!(
        host = %server.host,
        port = server.port,
        revision,
        "connected to the dail server"
    );
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
                    match client.state().await {
                        Ok((next, snapshot)) if next >= revision => {
                            revision = next;
                            if let Err(err) = session.ingest(synced(next, snapshot, true)).await {
                                return gone(err);
                            }
                        }
                        Ok(_) => {}
                        Err(err) => return Ended::Lost(err.to_string()),
                    }
                    // Answered once the engine has it: inputs are taken in order.
                    if session.flush().await.is_err() {
                        return Ended::Gone;
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
                if pinned(&states.borrow_and_update()).as_ref() != Some(server) {
                    tracing::info!(host = %server.host, port = server.port, "dail server link removed");
                    return Ended::Unlinked;
                }
            }
        }
    }
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

    /// The server's copy of blob `id`, while online.
    async fn server_blob(&self, id: &str) -> Result<Option<Vec<u8>>, SyncError> {
        match self.link.client() {
            Some(client) => client.blob(id).await,
            None => Ok(None),
        }
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
}

/// A remote profile's session as the front ends reach it (`Connection::Remote`): the local
/// session, with the commands for the server forwarded over the link (see the module doc).
#[derive(Clone)]
pub struct RemoteSession {
    local: SessionClient,
    link: Arc<RemoteLink>,
    seeds: Arc<dyn SeedSource>,
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
    ) -> Self {
        Self { local, link, seeds }
    }

    /// The local session (the mirror).
    pub const fn local(&self) -> &SessionClient {
        &self.local
    }

    /// The link to the server.
    pub const fn link(&self) -> &Arc<RemoteLink> {
        &self.link
    }

    /// Whether the profile is linked to a server it can connect to (a pinned server).
    fn linked(&self) -> bool {
        pinned(&self.local.state()).is_some()
    }

    /// Dispatches a command where its placement says (see the module doc). The outer error is
    /// the runtime's (the session is gone, the server is not connected: `ERR212`, or the link
    /// broke: `ERR211`); the inner one the engine's rejection, the server's for commands that
    /// run there.
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
        let Some(client) = client else {
            let status = self
                .local
                .state()
                .link
                .as_ref()
                .map_or(LinkStatus::Offline, |link| link.status);
            return Err(SyncError::NotOnline {
                command: cmd.name(),
                status: status_name(status),
            });
        };
        // Refused before anything is applied: the server would refuse it too.
        claimable(&client, origin)?;
        let to_server = match (&cmd, placement) {
            (Command::AttachFile { draft, path, index }, Placement::ServerWithUpload) => {
                Command::AttachFile {
                    draft: *draft,
                    path: upload(&client, path).await?,
                    index: *index,
                }
            }
            _ => cmd.clone(),
        };
        let seed = self.seeds.next_seed();
        let optimistic = self.local.submit_seeded(cmd, origin, seed).await?;
        match client
            .submit(&to_server, origin, request_id(seed), seed)
            .await
        {
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
                // Whether the server applied it is unknown: the next snapshot says.
                self.link.reconnect();
                Err(err)
            }
        }
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

    /// Waits until the local effects issued so far are done and, while online, the server's
    /// too, and the mirror has the server's state after them.
    pub async fn flush(&self) -> Result<(), SyncError> {
        self.local.flush().await?;
        if !self.linked() {
            return Ok(());
        }
        let Some(client) = self.link.client() else {
            return Ok(());
        };
        if let Err(err) = client.flush().await {
            if is_lost(&err) {
                self.link.reconnect();
            }
            return Err(err);
        }
        self.link.sync().await;
        self.local.flush().await
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

/// Uploads the file at `path` for `attach_file` and returns its path on the server.
async fn upload(client: &RemoteClient, path: &str) -> Result<String, SyncError> {
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
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|err| SyncError::Remote(format!("cannot read {path}: {err}")))?;
    client
        .put_file(app_core::draft::file_name(path), &bytes)
        .await
}
