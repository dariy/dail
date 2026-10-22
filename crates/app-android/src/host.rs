//! `Host`: one session and its presenter, driven by Kotlin's calls (ADR 0009).

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use app_core::{AppState, Command, Origin};
use app_io::{ProfileDirs, ProfileStore};
use app_mobile::{External, Frame, Mobile, Purpose, Reply, Request, UiEvent};
use app_sync::{
    AuditLog, Backend, Bridge, LiveBackend, NetImap, NetVerifier, ProfileLock, RandomSeeds,
    Session, SessionClient, SystemClock, Wake,
};
use serde::Serialize;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use crate::config::{BackendChoice, BlobRequest, Config, DebugTask, Task};
use crate::error::HostError;

/// The most a `background` task waits (a short service's 3 minutes).
pub const BACKGROUND_LIMIT: Duration = Duration::from_secs(180);

/// What `event` and `frame` return: the next screen and what only Android can do.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Output {
    /// The next screen.
    pub frame: Frame,
    /// Host actions asked for since the last call.
    pub externals: Vec<External>,
}

/// The session, its bridge and the presenter. Kotlin calls `event` and `frame` on one thread,
/// `blob` and `background` on IO threads; none of them blocks on the network but `background`.
pub struct Host {
    runtime: Runtime,
    bridge: Mutex<Bridge<Purpose>>,
    client: SessionClient,
    mobile: Mutex<Mobile>,
    task: Mutex<Option<JoinHandle<()>>>,
    _lock: Option<ProfileLock>,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host").finish_non_exhaustive()
    }
}

/// A poisoned lock still holds consistent data here: every critical section is a whole call.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn runtime() -> Result<Runtime, HostError> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("dail-rt")
        .enable_all()
        .build()?)
}

impl Host {
    /// Opens the profile (or the demo) and its session. `wake` is called from runtime threads
    /// whenever a new frame is ready.
    pub fn open(config: &Config, wake: Wake) -> Result<Self, HostError> {
        let runtime = runtime()?;
        let (session, lock) = match &config.backend {
            BackendChoice::Live => {
                let dirs = ProfileDirs::at(&config.profile_dir);
                dirs.ensure()?;
                let lock = ProfileLock::try_acquire(&dirs.lock_file())?
                    .ok_or_else(|| HostError::Locked(config.profile_dir.display().to_string()))?;
                let session = runtime.block_on(open_live(&dirs))?;
                (session, Some(lock))
            }
            BackendChoice::Memory(fixture) => (runtime.block_on(open_demo(fixture))?, None),
        };
        Ok(Self::start(runtime, session, lock, wake))
    }

    /// Runs over an open `session` (opened on `runtime`), as the app with `Origin::Gui`.
    pub fn with_session(runtime: Runtime, session: Session, wake: Wake) -> Self {
        Self::start(runtime, session, None, wake)
    }

    fn start(runtime: Runtime, session: Session, lock: Option<ProfileLock>, wake: Wake) -> Self {
        let client = session.client();
        let (bridge, task) = Bridge::own(runtime.handle(), session, Origin::Gui, wake);
        Self {
            runtime,
            bridge: Mutex::new(bridge),
            client,
            mobile: Mutex::new(Mobile::default()),
            task: Mutex::new(Some(task)),
            _lock: lock,
        }
    }

    /// The current state.
    pub fn state(&self) -> Arc<AppState> {
        lock(&self.bridge).state()
    }

    /// One event from Kotlin: submits what it asks for and returns the next frame.
    pub fn event(&self, event: UiEvent) -> Output {
        let bridge = lock(&self.bridge);
        let mut mobile = lock(&self.mobile);
        let requests = mobile.event(event, &bridge.state());
        submit(&bridge, requests);
        next(&bridge, &mut mobile)
    }

    /// [`Host::event`] over JSON.
    pub fn event_json(&self, event: &str) -> Result<Output, HostError> {
        Ok(self.event(serde_json::from_str(event)?))
    }

    /// The next frame, after a wake-up: feeds the replies that arrived to the presenter.
    pub fn frame(&self) -> Output {
        let bridge = lock(&self.bridge);
        let mut mobile = lock(&self.mobile);
        next(&bridge, &mut mobile)
    }

    /// Bytes of a stored part: as stored, or for an image its width and height (two
    /// little-endian `u32`) and then RGBA. `None` if it cannot be read or decoded. Blocks.
    pub fn blob(&self, request: &BlobRequest) -> Option<Vec<u8>> {
        let client = self.client.clone();
        self.runtime.block_on(async move {
            let (bytes, decode) = match request {
                BlobRequest::Attachment { blob, part } => {
                    (client.attachment_part(blob, *part).await.ok()??.1, false)
                }
                BlobRequest::Image { blob, part } => {
                    (client.attachment_part(blob, *part).await.ok()??.1, true)
                }
                BlobRequest::Inline { blob, cid } => {
                    (client.inline_part(blob, cid).await.ok()??, true)
                }
            };
            if !decode {
                return Some(bytes.bytes);
            }
            let image = tokio::task::spawn_blocking(move || app_io::images::decode(&bytes.bytes))
                .await
                .ok()?
                .ok()?;
            let mut out = Vec::with_capacity(8 + image.pixels.len());
            out.extend_from_slice(&image.width.to_le_bytes());
            out.extend_from_slice(&image.height.to_le_bytes());
            out.extend_from_slice(&image.pixels);
            Some(out)
        })
    }

    /// Runs `task` until it is done or `limit` passed; `Ok(true)` when it finished. Blocks.
    pub fn background(&self, task: Task, limit: Duration) -> Result<bool, HostError> {
        let client = self.client.clone();
        self.runtime.block_on(async move {
            let work = async {
                match task {
                    Task::Check => {
                        // A rejection (no accounts) leaves nothing to wait for.
                        let _ = client
                            .submit(Command::SyncNow { account: None }, Origin::Runtime)
                            .await?;
                        let mut state = client.subscribe();
                        state
                            .wait_for(|s| s.status.sync.values().all(|status| status.pending == 0))
                            .await
                            .map_err(|_| HostError::Closed)?;
                        Ok(())
                    }
                    Task::Flush => Ok::<(), HostError>(client.flush().await?),
                }
            };
            match tokio::time::timeout(limit, work).await {
                Ok(result) => result.map(|()| true),
                Err(_) => Ok(false),
            }
        })
    }

    /// A task of the debug adb bridge: `{"receipt": …}` or `{"rejected": …}` for `exec`, the
    /// state for `state`. Release builds refuse it, and `exec` takes only `cli` and `mcp`, so the
    /// bridge can never speak as the app (`gui`) or the runtime. Blocks.
    pub fn debug(&self, task: DebugTask) -> Result<serde_json::Value, HostError> {
        if !cfg!(debug_assertions) {
            return Err(HostError::Debug("not in release builds"));
        }
        match task {
            DebugTask::State => Ok(serde_json::to_value(&*self.state())?),
            DebugTask::Exec { origin, command } => {
                if !matches!(origin, Origin::Cli | Origin::Mcp) {
                    return Err(HostError::Debug("the origin must be cli or mcp"));
                }
                let command = match Command::from_value(command) {
                    Ok(command) => command,
                    Err(invalid) => return Ok(serde_json::json!({ "rejected": invalid })),
                };
                let client = self.client.clone();
                let result = self
                    .runtime
                    .block_on(async move { client.submit(command, origin).await })?;
                Ok(match result {
                    Ok(receipt) => serde_json::json!({ "receipt": receipt }),
                    Err(rejected) => serde_json::json!({ "rejected": rejected }),
                })
            }
        }
    }

    /// Shuts the session down after the queued requests (saving unsaved drafts) and waits.
    /// Later calls do nothing.
    pub fn close(&self) {
        lock(&self.bridge).close();
        let task = lock(&self.task).take();
        if let Some(task) = task
            && let Err(err) = self.runtime.block_on(task)
        {
            tracing::warn!(error = %err, "session task failed");
        }
    }
}

fn submit(bridge: &Bridge<Purpose>, requests: Vec<Request>) {
    for request in requests {
        bridge.submit(request.purpose, request.cmd);
    }
}

/// Feeds the replies, submits what they and the state call for, and draws.
fn next(bridge: &Bridge<Purpose>, mobile: &mut Mobile) -> Output {
    let replies = bridge
        .replies()
        .into_iter()
        .map(|reply| Reply {
            purpose: reply.tag,
            name: reply.name,
            result: reply.result.map_err(|err| err.to_string()),
        })
        .collect::<Vec<_>>();
    let state = bridge.state();
    if !replies.is_empty() {
        let requests = mobile.replies(replies, &state);
        submit(bridge, requests);
    }
    submit(bridge, mobile.refresh(&state));
    Output {
        frame: mobile.frame(&state),
        externals: mobile.externals(),
    }
}

/// The profile's session, as `app-cli` opens it: store, keyring, servers, the full-text index
/// and the audit log.
async fn open_live(dirs: &ProfileDirs) -> Result<Session, HostError> {
    let store = ProfileStore::open(dirs)?;
    let backend = LiveBackend::new(
        Arc::new(store),
        crate::secrets::store()?,
        Arc::new(NetVerifier::new()?),
        Arc::new(NetImap::new()?),
    );
    // Without an index, search still matches headers and asks the servers.
    let backend: Arc<dyn Backend> =
        match app_io::fulltext::FullText::open_or_rebuild(&dirs.index_dir()) {
            Ok(index) => Arc::new(backend.with_fulltext(Arc::new(index))),
            Err(err) => {
                tracing::warn!(error = %err, "no full-text index");
                Arc::new(backend)
            }
        };
    let audit = AuditLog::new(dirs.logs_dir());
    Ok(Session::open_audited(
        backend,
        Arc::new(SystemClock),
        Arc::new(RandomSeeds),
        Some(audit),
    )
    .await?)
}

/// The demo: a fake server over `fixture`, its first account added and synced, the inbox open.
#[cfg(feature = "demo")]
async fn open_demo(fixture: &str) -> Result<Session, HostError> {
    use app_core::{MailboxRole, ProviderPreset, Secret};
    use app_testkit::{Fixture, MemoryBackend};

    let fixture = Fixture::builtin(fixture).map_err(|err| HostError::Demo(err.to_string()))?;
    let email = fixture
        .accounts
        .first()
        .map(|a| a.email.clone())
        .ok_or_else(|| HostError::Demo("the fixture has no account".to_owned()))?;
    let backend: Arc<dyn Backend> = Arc::new(MemoryBackend::from_fixture(fixture));
    let session = Session::open(backend, Arc::new(SystemClock), Arc::new(RandomSeeds)).await?;
    let add = Command::AddAccount {
        preset: ProviderPreset::Hostinger,
        email,
        display_name: None,
        password: Some(Secret::new("demo")),
        imap: None,
        smtp: None,
    };
    for cmd in [add, Command::SyncNow { account: None }] {
        let _ = session.submit(cmd, Origin::Cli).await?;
        session.flush().await?;
    }
    let inbox = session
        .state()
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Inbox))
        .map(|m| m.id);
    if let Some(id) = inbox {
        let _ = session
            .submit(Command::OpenMailbox { id }, Origin::Cli)
            .await?;
    }
    Ok(session)
}

#[cfg(not(feature = "demo"))]
async fn open_demo(_fixture: &str) -> Result<Session, HostError> {
    Err(HostError::NoBackend("memory"))
}
