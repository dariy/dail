//! The dail server (ADR 0017, `ARCHITECTURE.md §5.4`): the profile socket's JSON-RPC over TLS, for
//! devices paired with a code.
//!
//! - [`ServerIdentity`]: the P-256 key and self-signed certificate in `<profile>/serve/`.
//! - [`ServeHub`]: what the server knows about devices: the open pairing codes
//!   (`Effect::OpenPairing`), the paired devices and their token hashes (`config.toml`
//!   `[[serve.devices]]`), their live connections (closed at once by `Effect::RevokeDevice`) and
//!   the answers to their last submissions.
//! - [`DailServer`]: TLS listeners serving a session to paired devices.
//!
//! Protocol version [`PROTOCOL`], one JSON-RPC 2.0 request per line, answered in order. The first
//! request of a connection is `pair` or `hello`; anything else closes it.
//!
//! ```text
//! → pair {code}                      ← {token, device: {id, name, role}, protocol, instance}  (then closed)
//! → hello {token, protocol, client_version}
//!                                    ← {protocol, server_version, instance, device: {id, name, role}}
//! → submit {origin, command, request_id, seed}  ← {"ok": receipt} | {"rejected": CoreError}
//! → preview {origin, command}        ← {"ok": Preview} | {"rejected": CoreError}
//! → flush                            ← null
//! → state                            ← {revision, state: StateDiff (a snapshot)}
//! → watch {since?, instance?}        ← null, then only notifications:
//!                                    ← changed {revision, diff: StateDiff}
//!                                    ← resync {revision, state: StateDiff (a snapshot)}
//! → blob {id}                        ← {bytes: base64} | null
//! → put_file {name, bytes: base64}   ← {path}
//! ```
//!
//! `seed` is a `u128` as a decimal string. `pair` takes no other parameter in version 1 (the
//! name and role are those of `start_pairing`; unknown parameters are ignored).

mod cert;

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use app_core::diff::StateDiff;
use app_core::{
    AppState, Command, DeviceId, DeviceRole, Effect, Event, IdGen, Origin, RequestId, Secret,
};
use app_io::{ConfigStore, ServeDevice};
use base64ct::{Base64, Base64UrlUnpadded, Encoding as _};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tokio::io::AsyncWriteExt as _;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};
use tokio_rustls::TlsAcceptor;

pub use cert::{CERT_VALIDITY, ServerIdentity};

use crate::clock::{Clock, SeedSource};
use crate::error::SyncError;
use crate::ipc::{PreviewReply, SubmitParams, SubmitReply, internal};
use crate::rpc::{
    self, BoxWrite, INTERNAL_ERROR, INVALID_PARAMS, INVALID_REQUEST, LineReader, METHOD_NOT_FOUND,
    Request,
};
use crate::session::SessionClient;
use crate::tls::encode_hex;

/// The protocol version. In version 1 a client's `hello.protocol` must equal it.
pub const PROTOCOL: u32 = 1;

/// The largest file a device may upload with `put_file` (25 MiB, what most mail servers take
/// in one message).
pub const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

/// The answers to the last this many submissions of each device are kept, so that a resend
/// is answered without dispatching again.
pub const REPLAY_MEMORY: usize = 1024;

/// Wrong pairing codes from one address within [`WRONG_CODE_WINDOW_MS`] that burn every open
/// code.
pub const MAX_WRONG_CODES: usize = 5;

/// The window of [`MAX_WRONG_CODES`], in milliseconds.
pub const WRONG_CODE_WINDOW_MS: i64 = 60_000;

/// JSON-RPC error: the pairing code or the device token was refused, or the first request was
/// neither `pair` nor `hello`. The connection is closed after it.
pub const NOT_AUTHORIZED: i64 = -32001;
/// JSON-RPC error: the client's protocol version is not [`PROTOCOL`]. The connection is closed
/// after it.
pub const PROTOCOL_MISMATCH: i64 = -32002;
/// JSON-RPC error: a request or an upload is too large.
pub const TOO_LARGE: i64 = -32003;
/// JSON-RPC error: the device's role may not claim the submission's origin.
pub const ORIGIN_REFUSED: i64 = -32004;

/// The longest first line (`pair`, `hello`) before a device is authenticated.
const MAX_FIRST_LINE: usize = 16 * 1024;
/// The longest request line of an authenticated device: an upload in base64 and some room.
const MAX_LINE: usize = MAX_UPLOAD_BYTES / 3 * 4 + 64 * 1024;
/// How long the TLS handshake and the first request may take.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// `DeviceSeen` is reported at most this often per device.
const SEEN_EVERY_MS: i64 = 60_000;
/// States kept to answer `watch {since}` with a diff.
const HISTORY: usize = 16;

/// Whether a listen address stays on a private network: loopback, RFC 1918, CGNAT
/// (`100.64.0.0/10`, Tailscale) or a unique local IPv6 address (`fc00::/7`). The server listens
/// anywhere; for other addresses it warns.
pub fn is_private_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_loopback() || v4.is_private() || (a == 100 && (b & 0xc0) == 64)
        }
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_private_address(IpAddr::V4(v4)),
            None => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
        },
    }
}

/// Whether `role` may submit as `origin`: a user device as the GUI, the TUI, the CLI or a batch;
/// an agent only as MCP (`mcp_allow_send` too when the server runs with `--allow-send`).
pub const fn origin_allowed(role: DeviceRole, origin: Origin, allow_send: bool) -> bool {
    match role {
        DeviceRole::User => matches!(
            origin,
            Origin::Gui | Origin::Tui | Origin::Cli | Origin::Batch
        ),
        DeviceRole::Agent => {
            matches!(origin, Origin::Mcp) || (allow_send && matches!(origin, Origin::McpAllowSend))
        }
    }
}

/// Whether two byte strings are equal, in time that depends only on their lengths.
fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A device token: 32 bytes from the OS random source, and the text a device sends.
fn new_token() -> Result<([u8; 32], String), SyncError> {
    let mut token = [0u8; 32];
    getrandom::fill(&mut token)
        .map_err(|err| SyncError::Serve(format!("no OS randomness: {err}")))?;
    let text = Base64UrlUnpadded::encode_string(&token);
    Ok((token, text))
}

/// SHA-256 of a token's bytes, lower-case hex: what `config.toml` keeps.
fn token_hash(token: &[u8]) -> String {
    encode_hex(&Sha256::digest(token))
}

/// An open pairing code.
struct OpenCode {
    code: Secret,
    role: DeviceRole,
    name: String,
    expires_at_ms: i64,
}

/// The answers to a device's last submissions.
#[derive(Default)]
struct Replies {
    order: VecDeque<RequestId>,
    answers: HashMap<RequestId, Value>,
}

impl Replies {
    fn insert(&mut self, id: RequestId, answer: Value) {
        if self.answers.insert(id, answer).is_none() {
            self.order.push_back(id);
            if self.order.len() > REPLAY_MEMORY
                && let Some(oldest) = self.order.pop_front()
            {
                self.answers.remove(&oldest);
            }
        }
    }
}

#[derive(Default)]
struct HubState {
    codes: Vec<OpenCode>,
    wrong: HashMap<IpAddr, VecDeque<i64>>,
    devices: Vec<ServeDevice>,
    connections: HashMap<u64, (DeviceId, watch::Sender<bool>)>,
    next_connection: u64,
    seen: HashMap<DeviceId, i64>,
    replies: HashMap<DeviceId, Arc<tokio::sync::Mutex<Replies>>>,
}

/// What a dail server knows about devices (see the module doc). Shared by the effects that
/// open codes and revoke devices (`LiveBackend::with_serve`) and the listeners
/// ([`DailServer`]).
pub struct ServeHub {
    config: Arc<ConfigStore>,
    clock: Arc<dyn Clock>,
    state: Mutex<HubState>,
}

impl std::fmt::Debug for ServeHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServeHub").finish_non_exhaustive()
    }
}

impl ServeHub {
    /// A hub whose paired devices live in `config` (`[[serve.devices]]`).
    pub fn new(config: Arc<ConfigStore>, clock: Arc<dyn Clock>) -> Self {
        let devices = config.serve().devices;
        Self {
            config,
            clock,
            state: Mutex::new(HubState {
                devices,
                ..HubState::default()
            }),
        }
    }

    /// The paired devices.
    pub fn devices(&self) -> Vec<ServeDevice> {
        self.state.lock().devices.clone()
    }

    /// How many pairing codes are open (expired ones included until the next attempt).
    pub fn open_codes(&self) -> usize {
        self.state.lock().codes.len()
    }

    /// How many authenticated connections are open.
    pub fn connections(&self) -> usize {
        self.state.lock().connections.len()
    }

    /// Opens `code` once, until `expires_at_ms`, for a device with `role` and `name`.
    pub fn open_pairing(&self, code: Secret, role: DeviceRole, name: String, expires_at_ms: i64) {
        let code = Secret::new(code.expose().trim().to_ascii_uppercase());
        let mut state = self.state.lock();
        state
            .codes
            .retain(|open| open.code.expose() != code.expose());
        state.codes.push(OpenCode {
            code,
            role,
            name,
            expires_at_ms,
        });
    }

    /// Forgets device `id` (its token stops working at once), closes its connections and removes
    /// it from `config.toml`. Revoking an unknown device succeeds.
    pub async fn revoke(&self, id: DeviceId) -> Result<(), SyncError> {
        {
            let mut state = self.state.lock();
            state.devices.retain(|device| device.id != id);
            state.replies.remove(&id);
            state.seen.remove(&id);
            for (device, closing) in state.connections.values() {
                if *device == id {
                    closing.send_replace(true);
                }
            }
        }
        let config = Arc::clone(&self.config);
        tokio::task::spawn_blocking(move || {
            config.update_serve(|serve| serve.devices.retain(|device| device.id != id))
        })
        .await
        .map_err(|err| SyncError::Serve(format!("config task failed: {err}")))?
        .map_err(|err| SyncError::Serve(format!("cannot remove the device: {err}")))
    }

    /// Runs the server's effects (`OpenPairing`, `RevokeDevice`); `None` for any other.
    pub async fn execute(&self, effect: &Effect) -> Option<Result<Vec<Event>, SyncError>> {
        match effect {
            Effect::OpenPairing {
                code,
                role,
                name,
                expires_at_ms,
            } => {
                self.open_pairing(code.clone(), *role, name.clone(), *expires_at_ms);
                Some(Ok(Vec::new()))
            }
            Effect::RevokeDevice { id } => Some(self.revoke(*id).await.map(|()| Vec::new())),
            _ => None,
        }
    }

    /// Takes `code` if it is open and unexpired. A wrong one counts against `peer`; the
    /// [`MAX_WRONG_CODES`]th within [`WRONG_CODE_WINDOW_MS`] burns every open code.
    fn redeem(&self, code: &str, peer: IpAddr) -> Option<(DeviceRole, String)> {
        let now = self.clock.now_ms();
        let mut state = self.state.lock();
        state.codes.retain(|open| open.expires_at_ms > now);
        let found = state
            .codes
            .iter()
            .position(|open| same_bytes(open.code.expose().as_bytes(), code.as_bytes()));
        if let Some(index) = found {
            let open = state.codes.remove(index);
            return Some((open.role, open.name));
        }
        state
            .wrong
            .retain(|_, times| times.back().is_some_and(|t| now - t < WRONG_CODE_WINDOW_MS));
        let times = state.wrong.entry(peer).or_default();
        times.retain(|t| now - t < WRONG_CODE_WINDOW_MS);
        times.push_back(now);
        if times.len() >= MAX_WRONG_CODES {
            times.clear();
            state.codes.clear();
            tracing::warn!(
                %peer,
                "{MAX_WRONG_CODES} wrong pairing codes within a minute: every open code is burnt"
            );
        }
        None
    }

    /// Adds a paired device, in memory and in `config.toml`.
    async fn register(&self, device: ServeDevice) -> Result<(), SyncError> {
        let config = Arc::clone(&self.config);
        let row = device.clone();
        tokio::task::spawn_blocking(move || {
            config.update_serve(|serve| {
                serve.devices.retain(|d| d.id != row.id);
                serve.devices.push(row);
            })
        })
        .await
        .map_err(|err| SyncError::Serve(format!("config task failed: {err}")))?
        .map_err(|err| SyncError::Serve(format!("cannot save the device: {err}")))?;
        let mut state = self.state.lock();
        state.devices.retain(|d| d.id != device.id);
        state.devices.push(device);
        Ok(())
    }

    /// The device whose token is `token`, with its connection registered in the same critical
    /// section, so that a revocation either comes first (the token fails) or signals this
    /// connection. Every stored hash is compared, in constant time.
    fn authenticate_and_attach(
        &self,
        token: &str,
    ) -> Option<(ServeDevice, u64, watch::Receiver<bool>)> {
        let bytes = Base64UrlUnpadded::decode_vec(token).ok()?;
        if bytes.len() != 32 {
            return None;
        }
        let hash = token_hash(&bytes);
        let mut state = self.state.lock();
        let mut found = None;
        for device in &state.devices {
            if same_bytes(device.token_sha256.as_bytes(), hash.as_bytes()) {
                found = Some(device.clone());
            }
        }
        let device = found?;
        let (key, closing) = Self::attach_locked(&mut state, device.id)?;
        Some((device, key, closing))
    }

    /// Registers a connection of `id`, a device that must still be paired; the receiver turns
    /// `true` when the device is revoked.
    fn attach_locked(state: &mut HubState, id: DeviceId) -> Option<(u64, watch::Receiver<bool>)> {
        if !state.devices.iter().any(|device| device.id == id) {
            return None;
        }
        let (tx, rx) = watch::channel(false);
        let key = state.next_connection;
        state.next_connection += 1;
        state.connections.insert(key, (id, tx));
        Some((key, rx))
    }

    fn detach(&self, key: u64) {
        self.state.lock().connections.remove(&key);
    }

    /// Whether `DeviceSeen` is due for `id` now (at most once a minute); records it if so.
    fn seen_due(&self, id: DeviceId, now: i64) -> bool {
        let mut state = self.state.lock();
        let due = state
            .seen
            .get(&id)
            .is_none_or(|last| now - last >= SEEN_EVERY_MS);
        if due {
            state.seen.insert(id, now);
        }
        due
    }

    fn replies(&self, id: DeviceId) -> Arc<tokio::sync::Mutex<Replies>> {
        Arc::clone(self.state.lock().replies.entry(id).or_default())
    }
}

/// How a [`DailServer`] serves.
#[derive(Debug, Clone)]
pub struct ServeOptions {
    /// Whether agent devices may claim `mcp_allow_send` (`dail serve --allow-send`).
    pub allow_send: bool,
    /// Where `put_file` stores uploads (`<profile>/serve/uploads`), one directory per upload.
    pub uploads_dir: PathBuf,
}

impl ServeOptions {
    /// Uploads under `uploads_dir`, agents without `--allow-send`.
    pub const fn new(uploads_dir: PathBuf) -> Self {
        Self {
            allow_send: false,
            uploads_dir,
        }
    }
}

struct Shared {
    hub: Arc<ServeHub>,
    session: SessionClient,
    seeds: Arc<dyn SeedSource>,
    acceptor: TlsAcceptor,
    instance: String,
    options: ServeOptions,
    /// Recent states, oldest first, for `watch {since}`.
    history: Mutex<VecDeque<Arc<AppState>>>,
}

impl Shared {
    fn remember(&self, state: &Arc<AppState>) {
        let mut history = self.history.lock();
        let revision = state.revision.get();
        if history
            .back()
            .is_some_and(|last| last.revision.get() >= revision)
        {
            return;
        }
        history.push_back(Arc::clone(state));
        if history.len() > HISTORY {
            history.pop_front();
        }
    }

    fn recall(&self, revision: u64) -> Option<Arc<AppState>> {
        self.history
            .lock()
            .iter()
            .find(|state| state.revision.get() == revision)
            .cloned()
    }

    /// Reports that `id` connected, at most once a minute.
    async fn seen(&self, id: DeviceId) {
        let now = self.hub.clock.now_ms();
        if self.hub.seen_due(id, now)
            && let Err(err) = self
                .session
                .ingest(Event::DeviceSeen { id, at_ms: now })
                .await
        {
            tracing::debug!(error = %err, "device seen not recorded");
        }
    }
}

/// A dail server: TLS listeners that serve one session to paired devices. Dropping it stops
/// every listener and closes every connection.
pub struct DailServer {
    shared: Arc<Shared>,
    tasks: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for DailServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DailServer")
            .field("instance", &self.shared.instance)
            .finish_non_exhaustive()
    }
}

impl DailServer {
    /// A server with `identity`'s certificate over `session`. `seeds` gives the IDs of new
    /// devices and uploads (the session's seed source). Must be called inside a tokio runtime;
    /// [`DailServer::listen`] adds the addresses.
    pub fn start(
        identity: &ServerIdentity,
        hub: Arc<ServeHub>,
        session: SessionClient,
        seeds: Arc<dyn SeedSource>,
        options: ServeOptions,
    ) -> Result<Self, SyncError> {
        let acceptor = TlsAcceptor::from(identity.server_config()?);
        let shared = Arc::new(Shared {
            hub,
            session,
            seeds,
            acceptor,
            instance: uuid::Uuid::new_v4().to_string(),
            options,
            history: Mutex::new(VecDeque::new()),
        });
        let recorder = {
            let shared = Arc::clone(&shared);
            let mut changes = shared.session.subscribe();
            tokio::spawn(async move {
                loop {
                    let state = Arc::clone(&changes.borrow_and_update());
                    shared.remember(&state);
                    if changes.changed().await.is_err() {
                        break;
                    }
                }
            })
        };
        Ok(Self {
            shared,
            tasks: vec![recorder],
        })
    }

    /// The identifier of this server process, in `hello`'s answer: a device that sees another
    /// one knows the server restarted and its revisions started over.
    pub fn instance(&self) -> &str {
        &self.shared.instance
    }

    /// Listens on `addr` (port 0: any free port) and returns the bound address. An address
    /// outside private networks ([`is_private_address`]) is allowed, with a warning.
    pub async fn listen(&mut self, addr: SocketAddr) -> Result<SocketAddr, SyncError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|err| SyncError::Serve(format!("cannot listen on {addr}: {err}")))?;
        let bound = listener
            .local_addr()
            .map_err(|err| SyncError::Serve(format!("cannot listen on {addr}: {err}")))?;
        if !is_private_address(bound.ip()) {
            tracing::warn!(
                address = %bound,
                "the dail server listens outside loopback and private networks; only TLS and the device tokens protect it"
            );
        }
        let shared = Arc::clone(&self.shared);
        self.tasks.push(tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => match accepted {
                        Ok((stream, peer)) => {
                            connections.spawn(serve_connection(Arc::clone(&shared), stream, peer));
                        }
                        Err(err) => tracing::warn!(error = %err, "dail server accept failed"),
                    },
                    Some(_) = connections.join_next(), if !connections.is_empty() => {}
                }
            }
        }));
        tracing::info!(address = %bound, "dail server listening");
        Ok(bound)
    }
}

impl Drop for DailServer {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

async fn serve_connection(shared: Arc<Shared>, tcp: TcpStream, peer: SocketAddr) {
    let tls = match tokio::time::timeout(HANDSHAKE_TIMEOUT, shared.acceptor.accept(tcp)).await {
        Ok(Ok(tls)) => tls,
        Ok(Err(err)) => {
            tracing::debug!(%peer, error = %err, "TLS handshake failed");
            return;
        }
        Err(_) => {
            tracing::debug!(%peer, "TLS handshake timed out");
            return;
        }
    };
    let (recv, mut send) = rpc::split(tls);
    let mut lines = LineReader::new(recv);
    let first = match tokio::time::timeout(HANDSHAKE_TIMEOUT, lines.next_line(MAX_FIRST_LINE)).await
    {
        Ok(Ok(Some(line))) => line,
        _ => return,
    };
    let request = match serde_json::from_str::<Request>(&first) {
        Ok(request) => request,
        Err(err) => {
            let reply = rpc::error(Value::Null, INVALID_REQUEST, &format!("bad request: {err}"));
            close(send, Some(reply)).await;
            return;
        }
    };
    match request.method.as_str() {
        "pair" => {
            let reply = pair(&shared, request, peer.ip()).await;
            close(send, Some(reply)).await;
        }
        "hello" => match hello(&shared, &request) {
            Ok((attached, answer)) => {
                if rpc::write_line(&mut send, &answer).await.is_ok() {
                    authenticated(&shared, attached, lines, send).await;
                }
            }
            Err(reply) => close(send, Some(reply)).await,
        },
        _ => {
            let reply = rpc::error(
                request.id,
                NOT_AUTHORIZED,
                "the first request must be `hello` or `pair`",
            );
            close(send, Some(reply)).await;
        }
    }
}

/// Writes `reply` if any and closes the connection (TLS `close_notify`).
async fn close(mut send: BoxWrite, reply: Option<Value>) {
    if let Some(reply) = reply {
        let _ = rpc::write_line(&mut send, &reply).await;
    }
    let _ = send.shutdown().await;
}

#[derive(Deserialize)]
struct PairParams {
    code: String,
}

async fn pair(shared: &Shared, request: Request, peer: IpAddr) -> Value {
    let id = request.id;
    let Ok(params) = serde_json::from_value::<PairParams>(request.params) else {
        return rpc::error(id, INVALID_PARAMS, "bad pair params");
    };
    let code = params.code.trim().to_ascii_uppercase();
    let Some((role, name)) = shared.hub.redeem(&code, peer) else {
        return rpc::error(
            id,
            NOT_AUTHORIZED,
            "the pairing code is wrong, expired or already used",
        );
    };
    let (token, text) = match new_token() {
        Ok(token) => token,
        Err(err) => return rpc::error(id, INTERNAL_ERROR, &err.to_string()),
    };
    let device_id: DeviceId = IdGen::new(shared.seeds.next_seed()).next_id();
    let row = ServeDevice {
        id: device_id,
        name,
        role,
        token_sha256: token_hash(&token),
        paired_at_ms: shared.hub.clock.now_ms(),
    };
    let device = row.to_device();
    if let Err(err) = shared.hub.register(row).await {
        tracing::warn!(error = %err, "pairing failed");
        return rpc::error(id, INTERNAL_ERROR, &err.to_string());
    }
    if let Err(err) = shared
        .session
        .ingest(Event::DevicePaired {
            device: device.clone(),
        })
        .await
    {
        tracing::warn!(error = %err, "paired device not recorded in the state");
    }
    tracing::info!(device = %device_id, role = role.as_str(), "device paired");
    rpc::answer(
        id,
        json!({
            "token": text,
            "device": { "id": device.id, "name": device.name, "role": device.role },
            "protocol": PROTOCOL,
            "instance": shared.instance,
        }),
    )
}

#[derive(Deserialize)]
struct HelloParams {
    token: String,
    protocol: u32,
    #[serde(default)]
    client_version: String,
}

/// A connection registered with the hub ([`ServeHub::authenticate_and_attach`]); dropping it
/// unregisters the connection.
struct Attached<'a> {
    hub: &'a ServeHub,
    key: u64,
    device: ServeDevice,
    closing: watch::Receiver<bool>,
}

impl Drop for Attached<'_> {
    fn drop(&mut self) {
        self.hub.detach(self.key);
    }
}

/// Checks `hello`: the paired device, registered before anything is answered, and the answer;
/// or the error to close with.
fn hello<'a>(shared: &'a Shared, request: &Request) -> Result<(Attached<'a>, Value), Value> {
    let id = request.id.clone();
    let params = serde_json::from_value::<HelloParams>(request.params.clone())
        .map_err(|_| rpc::error(id.clone(), INVALID_PARAMS, "bad hello params"))?;
    if params.protocol != PROTOCOL {
        return Err(rpc::error(
            id,
            PROTOCOL_MISMATCH,
            &format!(
                "this server speaks protocol {PROTOCOL}, not {}",
                params.protocol
            ),
        ));
    }
    let (device, key, closing) = shared
        .hub
        .authenticate_and_attach(&params.token)
        .ok_or_else(|| rpc::error(id.clone(), NOT_AUTHORIZED, "unknown device token"))?;
    tracing::debug!(device = %device.id, client_version = %params.client_version, "device connected");
    let answer = rpc::answer(
        id,
        json!({
            "protocol": PROTOCOL,
            "server_version": env!("CARGO_PKG_VERSION"),
            "instance": shared.instance,
            "device": { "id": device.id, "name": device.name, "role": device.role },
        }),
    );
    let attached = Attached {
        hub: &shared.hub,
        key,
        device,
        closing,
    };
    Ok((attached, answer))
}

async fn revoked(closing: &mut watch::Receiver<bool>) {
    // A dropped sender (the hub forgot the connection) closes it too.
    let _ = closing.wait_for(|revoked| *revoked).await;
}

/// Serves an authenticated device until it closes the connection or is revoked.
async fn authenticated(
    shared: &Shared,
    mut attached: Attached<'_>,
    mut lines: LineReader<rpc::BoxRead>,
    mut send: BoxWrite,
) {
    // A revocation since `hello` has already flipped this signal: the loop ends at once.
    let device = attached.device.clone();
    let closing = &mut attached.closing;
    shared.seen(device.id).await;
    loop {
        let line = tokio::select! {
            line = lines.next_line(MAX_LINE) => line,
            () = revoked(closing) => break,
        };
        let line = match line {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(err) => {
                let reply = rpc::error(Value::Null, TOO_LARGE, &err.to_string());
                let _ = rpc::write_line(&mut send, &reply).await;
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let request = match serde_json::from_str::<Request>(&line) {
            Ok(request) => request,
            Err(err) => {
                let reply =
                    rpc::error(Value::Null, INVALID_REQUEST, &format!("bad request: {err}"));
                if rpc::write_line(&mut send, &reply).await.is_err() {
                    break;
                }
                continue;
            }
        };
        if request.method == "watch" {
            shared.seen(device.id).await;
            tokio::select! {
                () = watch_changes(shared, request, &mut send, &mut lines) => {}
                () = revoked(closing) => {}
            }
            break;
        }
        let id = request.id.clone();
        let outcome = tokio::select! {
            outcome = respond(shared, &device, request) => outcome,
            () = revoked(closing) => break,
        };
        let reply = match outcome {
            Ok(result) => {
                shared.seen(device.id).await;
                rpc::answer(id, result)
            }
            Err((code, message)) => rpc::error(id, code, &message),
        };
        if rpc::write_line(&mut send, &reply).await.is_err() {
            break;
        }
    }
    let _ = send.shutdown().await;
}

type Outcome = Result<Value, (i64, String)>;

async fn respond(shared: &Shared, device: &ServeDevice, request: Request) -> Outcome {
    match request.method.as_str() {
        "submit" => submit(shared, device, request.params).await,
        "preview" => preview(shared, device, request.params).await,
        "flush" => shared
            .session
            .flush()
            .await
            .map(|()| Value::Null)
            .map_err(internal),
        "state" => {
            let state = shared.session.state();
            shared.remember(&state);
            Ok(json!({
                "revision": state.revision.get(),
                "state": StateDiff::snapshot(&state),
            }))
        }
        "blob" => blob(shared, request.params).await,
        "put_file" => put_file(shared, request.params).await,
        "hello" | "pair" => Err((INVALID_REQUEST, "already connected".to_owned())),
        other => Err((METHOD_NOT_FOUND, format!("unknown method `{other}`"))),
    }
}

fn check_origin(
    shared: &Shared,
    device: &ServeDevice,
    origin: Origin,
) -> Result<(), (i64, String)> {
    if origin_allowed(device.role, origin, shared.options.allow_send) {
        return Ok(());
    }
    let origin = serde_json::to_value(origin)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    Err((
        ORIGIN_REFUSED,
        format!(
            "a device with role `{}` can't submit as `{origin}`",
            device.role.as_str()
        ),
    ))
}

#[derive(Deserialize)]
struct SubmitExtra {
    request_id: RequestId,
    seed: Value,
}

/// A `u128` seed written as a decimal string (or a JSON number).
fn parse_seed(value: &Value) -> Option<u128> {
    match value {
        Value::String(text) => text.parse().ok(),
        Value::Number(number) => number.as_u64().map(u128::from),
        _ => None,
    }
}

async fn submit(shared: &Shared, device: &ServeDevice, params: Value) -> Outcome {
    let extra: SubmitExtra = serde_json::from_value(params.clone())
        .map_err(|err| (INVALID_PARAMS, format!("bad submit params: {err}")))?;
    let seed = parse_seed(&extra.seed)
        .ok_or_else(|| (INVALID_PARAMS, "bad submit params: seed".to_owned()))?;
    let params: SubmitParams = serde_json::from_value(params)
        .map_err(|err| (INVALID_PARAMS, format!("bad submit params: {err}")))?;
    check_origin(shared, device, params.origin)?;
    // One submission of a device at a time, so that a resend waits for the first answer.
    let replies = shared.hub.replies(device.id);
    let mut replies = replies.lock().await;
    if let Some(answer) = replies.answers.get(&extra.request_id) {
        return Ok(answer.clone());
    }
    let reply = match Command::from_value(params.command) {
        Ok(cmd) => match shared
            .session
            .submit_seeded(cmd, params.origin, seed)
            .await
            .map_err(internal)?
        {
            Ok(receipt) => SubmitReply::Ok(receipt),
            Err(err) => SubmitReply::Rejected(err),
        },
        Err(err) => SubmitReply::Rejected(err),
    };
    let answer = serde_json::to_value(reply).map_err(|err| (INTERNAL_ERROR, err.to_string()))?;
    replies.insert(extra.request_id, answer.clone());
    Ok(answer)
}

async fn preview(shared: &Shared, device: &ServeDevice, params: Value) -> Outcome {
    let params: SubmitParams = serde_json::from_value(params)
        .map_err(|err| (INVALID_PARAMS, format!("bad preview params: {err}")))?;
    check_origin(shared, device, params.origin)?;
    let reply = match Command::from_value(params.command) {
        Ok(cmd) => match shared
            .session
            .preview(cmd, params.origin)
            .await
            .map_err(internal)?
        {
            Ok(preview) => PreviewReply::Ok(preview),
            Err(err) => PreviewReply::Rejected(err),
        },
        Err(err) => PreviewReply::Rejected(err),
    };
    serde_json::to_value(reply).map_err(|err| (INTERNAL_ERROR, err.to_string()))
}

#[derive(Deserialize)]
struct BlobParams {
    id: String,
}

async fn blob(shared: &Shared, params: Value) -> Outcome {
    let params: BlobParams = serde_json::from_value(params)
        .map_err(|err| (INVALID_PARAMS, format!("bad blob params: {err}")))?;
    let valid = params.id.len() == 64
        && params
            .id
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !valid {
        return Err((INVALID_PARAMS, "bad blob id".to_owned()));
    }
    match shared.session.blob(&params.id).await.map_err(internal)? {
        Some(bytes) => Ok(json!({ "bytes": Base64::encode_string(&bytes) })),
        None => Ok(Value::Null),
    }
}

#[derive(Deserialize)]
struct PutFileParams {
    name: String,
    bytes: String,
}

async fn put_file(shared: &Shared, params: Value) -> Outcome {
    let params: PutFileParams = serde_json::from_value(params)
        .map_err(|err| (INVALID_PARAMS, format!("bad put_file params: {err}")))?;
    let too_large = || {
        (
            TOO_LARGE,
            format!(
                "the file is larger than {} MiB",
                MAX_UPLOAD_BYTES / (1024 * 1024)
            ),
        )
    };
    if params.bytes.len() > MAX_UPLOAD_BYTES.div_ceil(3) * 4 {
        return Err(too_large());
    }
    let bytes = Base64::decode_vec(&params.bytes)
        .map_err(|_| (INVALID_PARAMS, "bad put_file params: bytes".to_owned()))?;
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(too_large());
    }
    let name = app_io::mime::safe_file_name(Some(&params.name), 1);
    let folder = uuid::Uuid::from_u128(shared.seeds.next_seed()).to_string();
    let dir = shared.options.uploads_dir.join(folder);
    let path = dir.join(name);
    let target = path.clone();
    tokio::task::spawn_blocking(move || {
        cert::ensure_private_dir(&dir)?;
        cert::write_private(&target, &bytes)
    })
    .await
    .map_err(|err| (INTERNAL_ERROR, err.to_string()))?
    .map_err(internal)?;
    Ok(json!({ "path": path.display().to_string() }))
}

#[derive(Deserialize, Default)]
struct WatchParams {
    #[serde(default)]
    since: Option<u64>,
    #[serde(default)]
    instance: Option<String>,
}

/// After `watch`: `changed` with the diff after each burst of changes, starting with the change
/// since `since` (or `resync` with a snapshot when that state is unknown, or the client saw
/// another server instance).
async fn watch_changes(
    shared: &Shared,
    request: Request,
    send: &mut BoxWrite,
    lines: &mut LineReader<rpc::BoxRead>,
) {
    let params: WatchParams = serde_json::from_value(request.params).unwrap_or_default();
    let mut changes = shared.session.subscribe();
    let current = Arc::clone(&changes.borrow_and_update());
    shared.remember(&current);
    if rpc::write_line(send, &rpc::answer(request.id, Value::Null))
        .await
        .is_err()
    {
        return;
    }
    let same_instance = params
        .instance
        .as_deref()
        .is_none_or(|instance| instance == shared.instance);
    let base = params
        .since
        .filter(|_| same_instance)
        .and_then(|since| shared.recall(since));
    let first = match base {
        Some(base) if base.revision.get() == current.revision.get() => None,
        Some(base) => Some(changed(&base, &current)),
        None => Some(rpc::notification(
            "resync",
            json!({
                "revision": current.revision.get(),
                "state": StateDiff::snapshot(&current),
            }),
        )),
    };
    if let Some(first) = first
        && rpc::write_line(send, &first).await.is_err()
    {
        return;
    }
    let mut last = current;
    loop {
        tokio::select! {
            result = changes.changed() => {
                if result.is_err() {
                    break;
                }
                let next = Arc::clone(&changes.borrow_and_update());
                shared.remember(&next);
                if StateDiff::diff(&last, &next).is_empty() {
                    last = next;
                    continue;
                }
                if rpc::write_line(send, &changed(&last, &next)).await.is_err() {
                    break;
                }
                last = next;
            }
            line = lines.next_line(MAX_FIRST_LINE) => match line {
                // A watching client has nothing to say; anything it sends is ignored.
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            },
        }
    }
}

fn changed(old: &AppState, new: &AppState) -> Value {
    rpc::notification(
        "changed",
        json!({ "revision": new.revision.get(), "diff": StateDiff::diff(old, new) }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn private_addresses_are_told_apart() {
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.10",
            "100.64.0.1",
            "100.127.255.254",
            "::1",
            "fd7a:115c:a1e0::1",
            "fc00::1",
            "::ffff:192.168.0.2",
        ] {
            assert!(is_private_address(ip(private)), "{private}");
        }
        for public in [
            "0.0.0.0",
            "8.8.8.8",
            "100.63.255.255",
            "100.128.0.1",
            "172.32.0.1",
            "::",
            "2001:db8::1",
            "fe80::1",
            "::ffff:8.8.8.8",
        ] {
            assert!(!is_private_address(ip(public)), "{public}");
        }
    }

    #[test]
    fn roles_claim_their_origins_only() {
        use DeviceRole::{Agent, User};
        for origin in [Origin::Gui, Origin::Tui, Origin::Cli, Origin::Batch] {
            assert!(origin_allowed(User, origin, false));
            assert!(!origin_allowed(Agent, origin, true));
        }
        assert!(!origin_allowed(User, Origin::Mcp, true));
        assert!(origin_allowed(Agent, Origin::Mcp, false));
        assert!(!origin_allowed(Agent, Origin::McpAllowSend, false));
        assert!(origin_allowed(Agent, Origin::McpAllowSend, true));
        assert!(!origin_allowed(User, Origin::Runtime, true));
        assert!(!origin_allowed(Agent, Origin::Rule, true));
    }

    #[test]
    fn replies_keep_the_last_ones() {
        let mut replies = Replies::default();
        for n in 0..=REPLAY_MEMORY as u128 {
            replies.insert(RequestId::from_uuid(uuid::Uuid::from_u128(n)), json!(n));
        }
        assert_eq!(replies.answers.len(), REPLAY_MEMORY);
        assert!(
            !replies
                .answers
                .contains_key(&RequestId::from_uuid(uuid::Uuid::from_u128(0)))
        );
    }

    fn hub(name: &str) -> ServeHub {
        let dir = std::env::temp_dir().join(format!("dail-hub-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = Arc::new(ConfigStore::open(dir.join("config.toml")).unwrap());
        ServeHub::new(config, Arc::new(crate::clock::SystemClock))
    }

    fn paired(hub: &ServeHub, n: u128) -> (DeviceId, String) {
        let id = DeviceId::from_uuid(uuid::Uuid::from_u128(n));
        let token = [u8::try_from(n).unwrap(); 32];
        hub.state.lock().devices.push(ServeDevice {
            id,
            name: "Phone".to_owned(),
            role: DeviceRole::User,
            token_sha256: token_hash(&token),
            paired_at_ms: 0,
        });
        (id, Base64UrlUnpadded::encode_string(&token))
    }

    #[tokio::test]
    async fn a_revocation_reaches_a_connection_attached_with_its_token() {
        let hub = hub("attach");
        let (id, token) = paired(&hub, 1);
        // Attached before the revocation (e.g. while the hello answer is written): signalled.
        let (device, _key, closing) = hub.authenticate_and_attach(&token).unwrap();
        assert_eq!(device.id, id);
        assert!(!*closing.borrow());
        hub.revoke(id).await.unwrap();
        assert!(*closing.borrow());
        // After it: refused, and the device can't be attached by its ID either.
        assert!(hub.authenticate_and_attach(&token).is_none());
        assert!(ServeHub::attach_locked(&mut hub.state.lock(), id).is_none());
        assert_eq!(
            hub.connections(),
            1,
            "until the signalled connection detaches"
        );
    }

    #[test]
    fn bytes_compare_in_full() {
        assert!(same_bytes(b"abc", b"abc"));
        assert!(!same_bytes(b"abc", b"abd"));
        assert!(!same_bytes(b"abc", b"ab"));
    }
}
