//! Single instance & IPC attach (`ARCHITECTURE.md §5.4`).
//!
//! The process that opens a profile holds its lock ([`ProfileLock`], `<profile>/lock`). A
//! long-running owner (the GUI, `dail mcp`) also listens on the profile's local socket
//! ([`IpcServer`]: a Unix domain socket `<profile>/dail.sock`, or a named pipe on Windows), so
//! `dail exec` / `batch` / `mcp` started later attach to it ([`IpcClient`]) and drive the same
//! state the user is looking at.
//!
//! The protocol is newline-delimited JSON-RPC 2.0, one request per line, answered in order:
//!
//! ```text
//! → {"jsonrpc":"2.0","id":1,"method":"submit","params":{"origin":"cli","command":{"cmd":"undo","args":{}}}}
//! ← {"jsonrpc":"2.0","id":1,"result":{"ok":{…receipt…}}}        (or {"rejected":{…CoreError…}})
//! → {"jsonrpc":"2.0","id":5,"method":"preview","params":{…as submit…}}  ← {"result":{"ok":{…Preview…}}}  (a dry run)
//! → {"jsonrpc":"2.0","id":2,"method":"flush"}                    ← {"jsonrpc":"2.0","id":2,"result":null}
//! → {"jsonrpc":"2.0","id":3,"method":"state"}                    ← {"jsonrpc":"2.0","id":3,"result":{…AppState…}}
//! → {"jsonrpc":"2.0","id":4,"method":"watch"}                    ← {"jsonrpc":"2.0","id":4,"result":null}
//!                                                                 ← {"jsonrpc":"2.0","method":"changed"}  (after changes)
//! ```
//!
//! After `watch` the connection only carries `changed` notifications: one after each burst of state
//! changes (coalesced), until either side closes it. A client watches on a connection of its own
//! and reads `state` again on its request connection ([`IpcClient::subscribe`]).
//!
//! Only the `tui`, `cli`, `batch`, `mcp` and `mcp_allow_send` origins may be claimed by a client: a local process
//! can't pose as the GUI or as the runtime. (`tui` and `cli` may approve agent sends, like the GUI; a process that
//! can open the user's socket acts as the user anyway, `ARCHITECTURE.md §9`.)

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use app_core::{AppState, Command, CoreError, Origin};
use interprocess::local_socket::tokio::{RecvHalf, SendHalf, Stream};
use interprocess::local_socket::traits::tokio::{Listener as _, Stream as _};
use interprocess::local_socket::{ListenerOptions, Name};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::sync::{Mutex, watch};
use tokio::task::{JoinHandle, JoinSet};

use crate::error::SyncError;
use crate::session::{SessionClient, SubmitResult};

/// How long [`claim`] waits for a busy profile (another process holds the lock but does not
/// listen yet, or is a short `dail exec`).
pub const CLAIM_TIMEOUT: Duration = Duration::from_secs(10);

/// Pause between attempts while a profile is busy.
const CLAIM_RETRY: Duration = Duration::from_millis(50);

/// JSON-RPC error codes.
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;

/// The exclusive lock of a profile directory, held while the process owns the profile. The OS
/// releases it when the file is closed, also when the process dies.
#[derive(Debug)]
pub struct ProfileLock {
    _file: File,
}

impl ProfileLock {
    /// Takes the lock on `path` (created if missing). `Ok(None)`: another process holds it.
    pub fn try_acquire(path: &Path) -> Result<Option<Self>, SyncError> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(|err| SyncError::Ipc(format!("cannot open {}: {err}", path.display())))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(err)) => Err(SyncError::Ipc(format!(
                "cannot lock {}: {err}",
                path.display()
            ))),
        }
    }
}

/// Where a profile's owner listens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcEndpoint {
    /// Unix: the socket path. Windows: the pipe name (`dail-<hash of the profile path>`).
    name: PathBuf,
}

impl IpcEndpoint {
    /// The endpoint of the profile rooted at `root`.
    pub fn for_profile(root: &Path) -> Self {
        #[cfg(windows)]
        {
            // Named pipes live in a global namespace: name them after the profile path.
            let hash = root
                .to_string_lossy()
                .bytes()
                .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
                });
            Self {
                name: PathBuf::from(format!("dail-{hash:016x}")),
            }
        }
        #[cfg(not(windows))]
        {
            Self {
                name: root.join("dail.sock"),
            }
        }
    }

    /// The socket path or pipe name, for diagnostics.
    pub fn display(&self) -> String {
        self.name.display().to_string()
    }

    fn to_name(&self) -> std::io::Result<Name<'_>> {
        #[cfg(windows)]
        {
            use interprocess::local_socket::{GenericNamespaced, ToNsName as _};
            self.name.as_os_str().to_ns_name::<GenericNamespaced>()
        }
        #[cfg(not(windows))]
        {
            use interprocess::local_socket::{GenericFilePath, ToFsName as _};
            self.name.as_path().to_fs_name::<GenericFilePath>()
        }
    }
}

/// Who gets to drive a profile.
#[derive(Debug)]
pub enum Claim {
    /// Another process owns the profile and listens: submit through this client.
    Attached(IpcClient),
    /// This process owns the profile now and must open it itself.
    Owned(ProfileLock),
}

/// Attaches to the owner of the profile at `root`, or takes its lock if nobody owns it. While
/// another process holds the lock without listening, retries until [`CLAIM_TIMEOUT`].
pub async fn claim(root: &Path) -> Result<Claim, SyncError> {
    let endpoint = IpcEndpoint::for_profile(root);
    let lock_path = root.join("lock");
    let deadline = tokio::time::Instant::now() + CLAIM_TIMEOUT;
    loop {
        if let Ok(client) = IpcClient::connect(&endpoint).await {
            return Ok(Claim::Attached(client));
        }
        if let Some(lock) = ProfileLock::try_acquire(&lock_path)? {
            return Ok(Claim::Owned(lock));
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(SyncError::ProfileBusy(root.display().to_string()));
        }
        tokio::time::sleep(CLAIM_RETRY).await;
    }
}

/// A request line.
#[derive(Debug, Serialize, Deserialize)]
struct Request {
    jsonrpc: String,
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

/// `submit` parameters.
#[derive(Debug, Deserialize)]
struct SubmitParams {
    origin: Origin,
    command: Value,
}

/// The `result` of `submit`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SubmitReply {
    Ok(app_core::CommandReceipt),
    Rejected(CoreError),
}

/// The `result` of `preview`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PreviewReply {
    Ok(app_core::Preview),
    Rejected(CoreError),
}

/// Origins a client may claim.
const fn origin_allowed(origin: Origin) -> bool {
    matches!(
        origin,
        Origin::Tui | Origin::Cli | Origin::Batch | Origin::Mcp | Origin::McpAllowSend
    )
}

/// A running listener. Dropping it stops accepting, closes every connection and removes the
/// socket file.
#[derive(Debug)]
pub struct IpcServer {
    endpoint: IpcEndpoint,
    task: JoinHandle<()>,
}

impl IpcServer {
    /// Listens on the profile's endpoint and serves `session` to every client. Call it only
    /// while holding the profile's [`ProfileLock`]; a stale socket file left by a crashed owner
    /// is replaced.
    pub fn listen(root: &Path, session: SessionClient) -> Result<Self, SyncError> {
        let endpoint = IpcEndpoint::for_profile(root);
        let name = endpoint.to_name().map_err(|err| {
            SyncError::Ipc(format!("bad socket name {}: {err}", endpoint.display()))
        })?;
        let listener = ListenerOptions::new()
            .name(name)
            .try_overwrite(true)
            .create_tokio()
            .map_err(|err| {
                SyncError::Ipc(format!("cannot listen on {}: {err}", endpoint.display()))
            })?;
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => match accepted {
                        Ok(stream) => {
                            connections.spawn(serve_connection(stream, session.clone()));
                        }
                        Err(err) => tracing::warn!(error = %err, "IPC accept failed"),
                    },
                    Some(_) = connections.join_next(), if !connections.is_empty() => {}
                }
            }
        });
        tracing::debug!(endpoint = %endpoint.display(), "IPC listening");
        Ok(Self { endpoint, task })
    }

    /// Where it listens.
    pub const fn endpoint(&self) -> &IpcEndpoint {
        &self.endpoint
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        // Aborting the task drops the listener (which removes the socket file) and the
        // connection set (which aborts every connection).
        self.task.abort();
    }
}

async fn serve_connection(stream: Stream, session: SessionClient) {
    let (recv, mut send) = stream.split();
    let mut lines = BufReader::new(recv).lines();
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(err) => {
                tracing::debug!(error = %err, "IPC connection closed");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(request) = serde_json::from_str::<Request>(&line)
            && request.method == "watch"
        {
            let ok = json!({ "jsonrpc": "2.0", "id": request.id, "result": null });
            if write_line(&mut send, &ok).await.is_ok() {
                notify_changes(lines, send, &session).await;
            }
            return;
        }
        let response = respond(&line, &session).await;
        let mut text = response.to_string();
        text.push('\n');
        if send.write_all(text.as_bytes()).await.is_err() {
            break;
        }
    }
}

async fn write_line(send: &mut SendHalf, value: &Value) -> std::io::Result<()> {
    let mut text = value.to_string();
    text.push('\n');
    send.write_all(text.as_bytes()).await
}

/// Sends `changed` after each burst of state changes until the session or the client goes away.
async fn notify_changes(
    mut lines: Lines<BufReader<RecvHalf>>,
    mut send: SendHalf,
    session: &SessionClient,
) {
    let mut changes = session.subscribe();
    changes.borrow_and_update();
    let changed = json!({ "jsonrpc": "2.0", "method": "changed" });
    loop {
        tokio::select! {
            result = changes.changed() => {
                // The watch channel keeps only the latest state: changes made while a
                // notification was being written are reported once.
                if result.is_err() || write_line(&mut send, &changed).await.is_err() {
                    break;
                }
            }
            line = lines.next_line() => match line {
                // A watching client has nothing to say; anything it sends is ignored.
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            },
        }
    }
}

async fn respond(line: &str, session: &SessionClient) -> Value {
    let request: Request = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(err) => return error(Value::Null, INVALID_REQUEST, &format!("bad request: {err}")),
    };
    let id = request.id;
    let outcome = match request.method.as_str() {
        "submit" => submit(request.params, session).await,
        "preview" => preview(request.params, session).await,
        "flush" => session
            .flush()
            .await
            .map(|()| Value::Null)
            .map_err(internal),
        "state" => serde_json::to_value(session.state().as_ref())
            .map_err(|err| (INTERNAL_ERROR, err.to_string())),
        other => Err((METHOD_NOT_FOUND, format!("unknown method `{other}`"))),
    };
    match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => error(id, code, &message),
    }
}

async fn submit(params: Value, session: &SessionClient) -> Result<Value, (i64, String)> {
    let params: SubmitParams = serde_json::from_value(params)
        .map_err(|err| (INVALID_PARAMS, format!("bad submit params: {err}")))?;
    if !origin_allowed(params.origin) {
        return Err((
            INVALID_PARAMS,
            format!("origin `{:?}` can't be claimed over IPC", params.origin).to_lowercase(),
        ));
    }
    let reply = match Command::from_value(params.command) {
        Ok(cmd) => match session.submit(cmd, params.origin).await.map_err(internal)? {
            Ok(receipt) => SubmitReply::Ok(receipt),
            Err(err) => SubmitReply::Rejected(err),
        },
        Err(err) => SubmitReply::Rejected(err),
    };
    serde_json::to_value(reply).map_err(|err| (INTERNAL_ERROR, err.to_string()))
}

async fn preview(params: Value, session: &SessionClient) -> Result<Value, (i64, String)> {
    let params: SubmitParams = serde_json::from_value(params)
        .map_err(|err| (INVALID_PARAMS, format!("bad preview params: {err}")))?;
    if !origin_allowed(params.origin) {
        return Err((
            INVALID_PARAMS,
            format!("origin `{:?}` can't be claimed over IPC", params.origin).to_lowercase(),
        ));
    }
    let reply = match Command::from_value(params.command) {
        Ok(cmd) => match session
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

fn internal(err: SyncError) -> (i64, String) {
    (INTERNAL_ERROR, err.to_string())
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// The wire form of a command. Unlike its `Serialize` form (used for receipts and logs), it
/// carries the password of `add_account`: the socket is private to the user, and the owner has
/// to hand the password to the keyring.
fn wire_command(cmd: &Command) -> Result<Value, SyncError> {
    let mut value =
        serde_json::to_value(cmd).map_err(|err| SyncError::Ipc(format!("bad command: {err}")))?;
    if let Command::AddAccount {
        password: Some(password),
        ..
    } = cmd
    {
        value["args"]["password"] = json!(password.expose());
    }
    Ok(value)
}

/// A connection to the owner of a profile.
#[derive(Debug, Clone)]
pub struct IpcClient {
    endpoint: IpcEndpoint,
    inner: Arc<Mutex<ClientConn>>,
}

struct ClientConn {
    lines: Lines<BufReader<RecvHalf>>,
    send: SendHalf,
    next_id: u64,
}

impl std::fmt::Debug for ClientConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientConn")
            .field("next_id", &self.next_id)
            .finish_non_exhaustive()
    }
}

impl IpcClient {
    /// Connects to `endpoint`; fails if nobody listens there.
    pub async fn connect(endpoint: &IpcEndpoint) -> Result<Self, SyncError> {
        let name = endpoint.to_name().map_err(|err| {
            SyncError::Ipc(format!("bad socket name {}: {err}", endpoint.display()))
        })?;
        let stream = Stream::connect(name).await.map_err(|err| {
            SyncError::Ipc(format!("cannot connect to {}: {err}", endpoint.display()))
        })?;
        let (recv, send) = stream.split();
        Ok(Self {
            endpoint: endpoint.clone(),
            inner: Arc::new(Mutex::new(ClientConn {
                lines: BufReader::new(recv).lines(),
                send,
                next_id: 1,
            })),
        })
    }

    /// Submits a command to the owner's session, like [`SessionClient::submit`].
    pub async fn submit(&self, cmd: &Command, origin: Origin) -> Result<SubmitResult, SyncError> {
        let params = json!({ "origin": origin, "command": wire_command(cmd)? });
        let result = self.call("submit", params).await?;
        match serde_json::from_value(result)
            .map_err(|err| SyncError::Ipc(format!("bad submit reply: {err}")))?
        {
            SubmitReply::Ok(receipt) => Ok(Ok(receipt)),
            SubmitReply::Rejected(err) => Ok(Err(err)),
        }
    }

    /// A dry run in the owner's session, like [`SessionClient::preview`].
    pub async fn preview(
        &self,
        cmd: &Command,
        origin: Origin,
    ) -> Result<crate::session::PreviewResult, SyncError> {
        let params = json!({ "origin": origin, "command": wire_command(cmd)? });
        let result = self.call("preview", params).await?;
        match serde_json::from_value(result)
            .map_err(|err| SyncError::Ipc(format!("bad preview reply: {err}")))?
        {
            PreviewReply::Ok(preview) => Ok(Ok(preview)),
            PreviewReply::Rejected(err) => Ok(Err(err)),
        }
    }

    /// Waits until the owner's in-flight effects are done, like [`SessionClient::flush`].
    pub async fn flush(&self) -> Result<(), SyncError> {
        self.call("flush", Value::Null).await.map(drop)
    }

    /// A snapshot of the owner's state.
    pub async fn state(&self) -> Result<AppState, SyncError> {
        let state = self.call("state", Value::Null).await?;
        serde_json::from_value(state).map_err(|err| SyncError::Ipc(format!("bad state: {err}")))
    }

    /// The owner's state, kept current: a second connection watches for `changed`
    /// notifications, and after each one the state is read again (one read at a time). The
    /// sender is dropped — `changed()` fails — once the owner goes away or this client's
    /// request connection breaks. Must be called inside a tokio runtime.
    pub async fn subscribe(&self) -> Result<watch::Receiver<Arc<AppState>>, SyncError> {
        let name = self.endpoint.to_name().map_err(|err| {
            SyncError::Ipc(format!(
                "bad socket name {}: {err}",
                self.endpoint.display()
            ))
        })?;
        let stream = Stream::connect(name).await.map_err(|err| {
            SyncError::Ipc(format!(
                "cannot connect to {}: {err}",
                self.endpoint.display()
            ))
        })?;
        let (recv, mut send) = stream.split();
        let mut lines = BufReader::new(recv).lines();
        let request = json!({ "jsonrpc": "2.0", "id": 0, "method": "watch" });
        write_line(&mut send, &request)
            .await
            .map_err(|_| SyncError::SessionClosed)?;
        match lines.next_line().await {
            Ok(Some(line))
                if serde_json::from_str::<Value>(&line)
                    .is_ok_and(|reply| reply.get("error").is_none()) => {}
            Ok(Some(line)) => return Err(SyncError::Ipc(format!("watch refused: {line}"))),
            Ok(None) | Err(_) => return Err(SyncError::SessionClosed),
        }
        // Read after the watch is in place, so no change between the two is missed.
        let (tx, rx) = watch::channel(Arc::new(self.state().await?));
        let client = self.clone();
        tokio::spawn(async move {
            // Keeps the write half open: closing it would end the watch.
            let _send = send;
            loop {
                let line = tokio::select! {
                    line = lines.next_line() => line,
                    // Every receiver is gone: stop watching.
                    () = tx.closed() => break,
                };
                let Ok(Some(line)) = line else {
                    break;
                };
                let notification: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
                if notification["method"] != "changed" {
                    continue;
                }
                match client.state().await {
                    Ok(state) => {
                        if tx.send(Arc::new(state)).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        tracing::debug!(error = %err, "state read after a change failed");
                        break;
                    }
                }
            }
        });
        Ok(rx)
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, SyncError> {
        let mut conn = self.inner.lock().await;
        let id = conn.next_id;
        conn.next_id += 1;
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        let mut text = request.to_string();
        text.push('\n');
        conn.send
            .write_all(text.as_bytes())
            .await
            .map_err(|_| SyncError::SessionClosed)?;
        let line = conn
            .lines
            .next_line()
            .await
            .map_err(|_| SyncError::SessionClosed)?
            .ok_or(SyncError::SessionClosed)?;
        let mut response: Value = serde_json::from_str(&line)
            .map_err(|err| SyncError::Ipc(format!("bad response: {err}")))?;
        if response["id"] != json!(id) {
            return Err(SyncError::Ipc("response out of order".to_owned()));
        }
        if let Some(error) = response.get("error") {
            let message = error["message"].as_str().unwrap_or("unknown error");
            return Err(SyncError::Ipc(message.to_owned()));
        }
        Ok(response["result"].take())
    }
}
