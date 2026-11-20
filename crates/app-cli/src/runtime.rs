//! Tokio runtime and session setup: attach to the profile's owner or open the profile here
//! (`ARCHITECTURE.md §5.4`).

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use app_core::{AppState, Effect, Event};
use app_io::{ConfigStore, ProfileDirs, ProfileStore};
use app_sync::ipc::{self, Claim};
use app_sync::serve::ServeHub;
use app_sync::{
    AuditLog, Backend, BoxFuture, Connection, IpcServer, KeyringSecrets, LiveBackend, NetImap,
    NetVerifier, ProfileLock, RandomSeeds, RemoteOptions, SecretStore, Session, SyncError,
    SystemClock,
};
use app_testkit::{Fixture, MemoryBackend, MemorySecrets};

use crate::cli::{BackendKind, GlobalArgs};

/// Runs `future` on a fresh multi-threaded runtime.
pub fn block_on<T>(future: impl Future<Output = anyhow::Result<T>>) -> anyhow::Result<T> {
    build()?.block_on(future)
}

/// A fresh multi-threaded runtime.
pub fn build() -> anyhow::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start the async runtime")
}

/// Whether a front-end that opens the profile itself accepts IPC clients while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listen {
    /// Long-running hosts (`dail mcp`, the GUI).
    Yes,
    /// One-shot runs (`exec`, `batch`, `account add`).
    No,
}

/// A session this process owns, with the profile lock and the IPC listener.
pub struct Owned {
    /// The session.
    pub session: Session,
    /// Accepts other dail processes while the session runs.
    pub server: Option<IpcServer>,
    /// Held until the session has shut down.
    pub lock: Option<ProfileLock>,
    /// The devices of `dail serve` (ADR 0017); none for every other owner.
    pub hub: Option<Arc<ServeHub>>,
}

impl Owned {
    /// Stops listening, then shuts the session down (unsaved drafts are saved) and releases the
    /// profile.
    pub async fn close(self) -> anyhow::Result<()> {
        drop(self.server);
        self.session.shutdown().await?;
        drop(self.lock);
        Ok(())
    }
}

/// Where a front-end's commands go, and the session to close afterwards if it is ours.
pub struct Front {
    /// Submit through this.
    pub conn: Connection,
    owned: Option<Owned>,
}

impl Front {
    /// Detaches, or shuts down the session this process owns.
    pub async fn close(self) -> anyhow::Result<()> {
        drop(self.conn);
        if let Some(owned) = self.owned {
            owned.close().await?;
        }
        Ok(())
    }
}

/// Who drives the selected profile.
pub enum Claimed {
    /// Another dail process owns it: submit through this.
    Attached(Connection),
    /// It was opened here.
    Owned(Owned),
}

/// Attaches to the process that owns the selected profile, or opens it here.
pub async fn claim(global: &GlobalArgs, listen: Listen) -> anyhow::Result<Claimed> {
    claim_serving(global, listen, Serve::No).await
}

/// [`claim`], opening the profile with `serve`'s hub if nobody owns it (`dail serve devices`
/// and `revoke` while `dail serve` is stopped: a revocation still reaches `config.toml`).
pub async fn claim_serving(
    global: &GlobalArgs,
    listen: Listen,
    serve: Serve,
) -> anyhow::Result<Claimed> {
    let root = profile_root(global)?;
    let lock = match &root {
        Some(root) => match ipc::claim(root).await? {
            Claim::Attached(client) => return Ok(Claimed::Attached(Connection::Attached(client))),
            Claim::Owned(lock) => Some(lock),
        },
        None => None,
    };
    Ok(Claimed::Owned(
        open_owned(global, root.as_deref(), lock, listen, serve).await?,
    ))
}

/// Attaches to the process that owns the selected profile, or opens it here.
pub async fn connect(global: &GlobalArgs, listen: Listen) -> anyhow::Result<Front> {
    connect_serving(global, listen, Serve::No).await
}

/// [`connect`] through [`claim_serving`].
pub async fn connect_serving(
    global: &GlobalArgs,
    listen: Listen,
    serve: Serve,
) -> anyhow::Result<Front> {
    Ok(match claim_serving(global, listen, serve).await? {
        Claimed::Attached(conn) => Front { conn, owned: None },
        Claimed::Owned(owned) => {
            // A remote profile (ADR 0017): give the link a moment to come up, so a one-shot
            // run's first command for the server is not refused while it is still dialing.
            if let Some(remote) = owned.session.remote()
                && let Some(status) = remote.wait_settled().await
            {
                tracing::info!(?status, "dail server link");
            }
            Front {
                conn: owned.session.connection(),
                owned: Some(owned),
            }
        }
    })
}

/// Whether the owner runs the dail server's effects (`dail serve`, ADR 0017).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serve {
    /// `dail serve`: pairing codes and revocations run on a [`ServeHub`].
    Yes,
    /// Every other owner: they fail with `ERR210` (live) or do nothing (memory).
    No,
}

/// Opens the selected profile in this process (the GUI, `dail serve`). Fails if another dail
/// process owns it.
pub async fn own(global: &GlobalArgs, serve: Serve) -> anyhow::Result<Owned> {
    let root = profile_root(global)?;
    let lock = match &root {
        Some(root) => match ipc::claim(root).await? {
            Claim::Attached(_) => bail!(
                "dail is already running for the profile {} (only one process may own a profile); close it first",
                root.display()
            ),
            Claim::Owned(lock) => Some(lock),
        },
        None => None,
    };
    open_owned(global, root.as_deref(), lock, Listen::Yes, serve).await
}

/// The profile directory whose lock and socket this run uses: the live profile, or
/// `--profile-dir` with `--backend memory` (a memory session is private otherwise).
pub fn profile_root(global: &GlobalArgs) -> anyhow::Result<Option<PathBuf>> {
    let profile = match (global.backend, &global.profile_dir) {
        (BackendKind::Memory, None) => return Ok(None),
        (_, Some(dir)) => ProfileDirs::at(dir),
        (BackendKind::Live, None) => ProfileDirs::named(&global.profile)?,
    };
    profile.ensure()?;
    Ok(Some(profile.root().to_path_buf()))
}

async fn open_owned(
    global: &GlobalArgs,
    root: Option<&Path>,
    lock: Option<ProfileLock>,
    listen: Listen,
    serve: Serve,
) -> anyhow::Result<Owned> {
    // A `listening.json` left by a `dail serve` that did not stop cleanly: this process owns
    // the profile now, so it is stale (`dail serve` writes a fresh one once it listens).
    if let Some(root) = root {
        let stale = crate::serve::listening_file(root);
        match std::fs::remove_file(&stale) {
            Ok(()) => tracing::info!(path = %stale.display(), "removed a stale listening.json"),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => bail!("cannot remove the stale {}: {err}", stale.display()),
        }
    }
    let mut hub = None;
    // Where a device keeps its token for a dail server (ADR 0017).
    let mut secrets: Arc<dyn SecretStore> = Arc::new(KeyringSecrets);
    let backend: Arc<dyn Backend> = match global.backend {
        BackendKind::Memory => {
            if serve == Serve::Yes {
                // The paired devices live in the profile's `config.toml`, as with a live profile.
                let root = root.context("`dail serve` needs a profile directory")?;
                let config = Arc::new(ConfigStore::open(ProfileDirs::at(root).config_file())?);
                hub = Some(Arc::new(ServeHub::new(config, Arc::new(SystemClock))));
            }
            let tokens = Arc::new(MemorySecrets::new());
            secrets = Arc::clone(&tokens) as Arc<dyn SecretStore>;
            Arc::new(MemoryServe {
                inner: MemoryBackend::from_fixture(load_fixture(global.fixture.as_deref())?),
                hub: hub.clone(),
                tokens,
            })
        }
        BackendKind::Live => {
            if global.fixture.is_some() {
                bail!("--fixture only applies to `--backend memory`");
            }
            let root = root.context("a live session needs a profile directory")?;
            let dirs = ProfileDirs::at(root);
            let store = ProfileStore::open(&dirs)?;
            let store_config = store.config();
            let backend = LiveBackend::new(
                Arc::new(store),
                Arc::new(KeyringSecrets),
                Arc::new(NetVerifier::new()?),
                Arc::new(NetImap::new()?),
            )
            .with_updates_dir(dirs.updates_dir())
            .with_draft_files_dir(dirs.draft_files_dir())
            .with_breaker(Arc::new(app_io::breaker::Breaker::open(dirs.breaker_dir())));
            let backend = if serve == Serve::Yes {
                // One `config.toml` store for the profile and the hub, so neither overwrites
                // the other.
                let serving = Arc::new(ServeHub::new(store_config, Arc::new(SystemClock)));
                hub = Some(Arc::clone(&serving));
                backend.with_serve(serving)
            } else {
                backend
            };
            // Without an index, search still matches headers and asks the servers.
            match app_io::fulltext::FullText::open_or_rebuild(&dirs.index_dir()) {
                Ok(index) => Arc::new(backend.with_fulltext(Arc::new(index))),
                Err(err) => {
                    tracing::warn!(error = %err, "no full-text index");
                    Arc::new(backend)
                }
            }
        }
    };
    // Sessions on a profile keep an audit log in its `logs/` (`dail replay`).
    let audit = root.map(|root| AuditLog::new(ProfileDirs::at(root).logs_dir()));
    let (clock, seeds) = (Arc::new(SystemClock), Arc::new(RandomSeeds));
    // Every owner but `dail serve` may be a device of a dail server: a remote profile's link
    // starts at once, a plain profile's once it is paired (ADR 0017).
    let session = match serve {
        Serve::Yes => Session::open_audited(backend, clock, seeds, audit).await?,
        Serve::No => {
            Session::open_remote(
                backend,
                secrets,
                clock,
                seeds,
                audit,
                RemoteOptions::default(),
            )
            .await?
        }
    };
    // A memory session starts empty: list the devices the profile's `config.toml` keeps, as a
    // live profile's store does.
    if let (BackendKind::Memory, Some(hub)) = (global.backend, &hub) {
        for device in hub.devices() {
            session
                .ingest(Event::DevicePaired {
                    device: device.to_device(),
                })
                .await?;
        }
    }
    // Attachments carried from server drafts that no draft names any more (MISC-44d).
    if let Some(root) = root.filter(|_| matches!(global.backend, BackendKind::Live)) {
        let dir = ProfileDirs::at(root).draft_files_dir();
        let removed = app_sync::draft_files::sweep(&dir, &session.state());
        if removed > 0 {
            tracing::info!(removed, "removed draft attachment folders no draft names");
        }
    }
    // The running build, for update checks (ADR 0011); a due periodic check starts now.
    let host = match global.backend {
        BackendKind::Live => app_sync::install::desktop_host(),
        BackendKind::Memory => app_testkit::updates::host(),
    };
    session
        .ingest(app_core::Event::UpdateHostDetected { host })
        .await?;
    let server = match (root, listen) {
        (Some(root), Listen::Yes) => Some(IpcServer::listen(root, session.connection())?),
        _ => None,
    };
    Ok(Owned {
        session,
        server,
        lock,
        hub,
    })
}

/// The memory backend as the CLI runs it (ADR 0001): fake mail servers, but a real dail server
/// (ADR 0017). Under `dail serve` pairing codes and revocations run on the hub; a device dials
/// a server for real (`connect_server`, `trust_server`) and keeps its token in memory only.
struct MemoryServe {
    inner: MemoryBackend,
    hub: Option<Arc<ServeHub>>,
    /// The device tokens, which the link reads too.
    tokens: Arc<MemorySecrets>,
}

impl Backend for MemoryServe {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>> {
        self.inner.load_state()
    }

    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>> {
        Box::pin(async move {
            match effect {
                Effect::OpenPairing { .. } | Effect::RevokeDevice { .. } => match &self.hub {
                    Some(hub) => hub.execute(&effect).await.unwrap_or(Ok(Vec::new())),
                    None => self.inner.execute(effect).await,
                },
                Effect::ProbeServer { host, port } => {
                    let fingerprint = app_sync::remote::probe(&host, port).await?;
                    Ok(vec![Event::ServerOffered {
                        host,
                        port,
                        fingerprint,
                    }])
                }
                Effect::PairWithServer {
                    host,
                    port,
                    fingerprint,
                    code,
                    secret,
                } => {
                    let paired = app_sync::remote::pair(&host, port, &fingerprint, &code).await?;
                    self.tokens.set(&secret, &paired.token)?;
                    Ok(vec![Event::ServerPaired { fingerprint }])
                }
                Effect::DisconnectServer { secret } => {
                    if let Some(secret) = &secret {
                        self.tokens.delete(secret)?;
                    }
                    self.inner
                        .execute(Effect::DisconnectServer { secret })
                        .await
                }
                effect => self.inner.execute(effect).await,
            }
        })
    }

    fn get_blob<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Option<Vec<u8>>, SyncError>> {
        self.inner.get_blob(id)
    }
}

fn load_fixture(fixture: Option<&str>) -> anyhow::Result<Fixture> {
    let Some(fixture) = fixture else {
        return Ok(Fixture::default());
    };
    let path = Path::new(fixture);
    let loaded = if fixture.ends_with(".json") || path.components().count() > 1 {
        Fixture::from_path(path)
    } else {
        Fixture::builtin(fixture)
    };
    Ok(loaded?)
}
