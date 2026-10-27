//! Tokio runtime and session setup: attach to the profile's owner or open the profile here
//! (`ARCHITECTURE.md §5.4`).

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use app_io::{ProfileDirs, ProfileStore};
use app_sync::ipc::{self, Claim};
use app_sync::{
    AuditLog, Backend, Connection, IpcServer, KeyringSecrets, LiveBackend, NetImap, NetVerifier,
    ProfileLock, RandomSeeds, Session, SystemClock,
};
use app_testkit::{Fixture, MemoryBackend};

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
    let root = profile_root(global)?;
    let lock = match &root {
        Some(root) => match ipc::claim(root).await? {
            Claim::Attached(client) => return Ok(Claimed::Attached(Connection::Attached(client))),
            Claim::Owned(lock) => Some(lock),
        },
        None => None,
    };
    Ok(Claimed::Owned(
        open_owned(global, root.as_deref(), lock, listen).await?,
    ))
}

/// Attaches to the process that owns the selected profile, or opens it here.
pub async fn connect(global: &GlobalArgs, listen: Listen) -> anyhow::Result<Front> {
    Ok(match claim(global, listen).await? {
        Claimed::Attached(conn) => Front { conn, owned: None },
        Claimed::Owned(owned) => Front {
            conn: Connection::Local(owned.session.client()),
            owned: Some(owned),
        },
    })
}

/// Opens the selected profile in this process (the GUI). Fails if another dail process owns
/// it.
pub async fn own(global: &GlobalArgs) -> anyhow::Result<Owned> {
    let root = profile_root(global)?;
    let lock = match &root {
        Some(root) => match ipc::claim(root).await? {
            Claim::Attached(_) => bail!(
                "dail is already running for the profile {}; close it first",
                root.display()
            ),
            Claim::Owned(lock) => Some(lock),
        },
        None => None,
    };
    open_owned(global, root.as_deref(), lock, Listen::Yes).await
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
) -> anyhow::Result<Owned> {
    let backend: Arc<dyn Backend> = match global.backend {
        BackendKind::Memory => Arc::new(MemoryBackend::from_fixture(load_fixture(
            global.fixture.as_deref(),
        )?)),
        BackendKind::Live => {
            if global.fixture.is_some() {
                bail!("--fixture only applies to `--backend memory`");
            }
            let root = root.context("a live session needs a profile directory")?;
            let dirs = ProfileDirs::at(root);
            let store = ProfileStore::open(&dirs)?;
            let backend = LiveBackend::new(
                Arc::new(store),
                Arc::new(KeyringSecrets),
                Arc::new(NetVerifier::new()?),
                Arc::new(NetImap::new()?),
            )
            .with_updates_dir(dirs.updates_dir());
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
    let session =
        Session::open_audited(backend, Arc::new(SystemClock), Arc::new(RandomSeeds), audit).await?;
    // The running build, for update checks (ADR 0011); a due periodic check starts now.
    let host = match global.backend {
        BackendKind::Live => app_sync::install::desktop_host(),
        BackendKind::Memory => app_testkit::updates::host(),
    };
    session
        .ingest(app_core::Event::UpdateHostDetected { host })
        .await?;
    let server = match (root, listen) {
        (Some(root), Listen::Yes) => Some(IpcServer::listen(root, session.client())?),
        _ => None,
    };
    Ok(Owned {
        session,
        server,
        lock,
    })
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
