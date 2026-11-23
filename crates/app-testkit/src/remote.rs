//! An in-process dail server for tests of remote profiles (ADR 0017): a [`MemoryBackend`]
//! session served over loopback TLS by a real [`DailServer`] with a [`ServeHub`], and devices
//! paired with it ([`TestServer::device`]): a session opened with
//! [`Session::open_remote`] over its own `MemoryBackend`, the token in [`MemorySecrets`].

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use app_core::{
    AppState, Command, DeviceRole, Effect, Event, LinkState, LinkStatus, Origin, Secret, SecretRef,
    ServerEndpoint,
};
use app_io::ConfigStore;
use app_sync::serve::{DailServer, ServeHub, ServeOptions, ServerIdentity};
use app_sync::{
    Backend, BoxFuture, Clock, RemoteOptions, SeedSource, Session, SyncError, SystemClock,
};

use crate::backend::MemoryBackend;
use crate::secrets::MemorySecrets;
use crate::store::MemoryStore;

/// The server's backend: the dail server's effects on the hub, everything else on the
/// [`MemoryBackend`].
struct Served {
    inner: Arc<MemoryBackend>,
    hub: Arc<ServeHub>,
}

impl Backend for Served {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>> {
        self.inner.load_state()
    }

    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>> {
        Box::pin(async move {
            match effect {
                Effect::OpenPairing { .. } | Effect::RevokeDevice { .. } => {
                    self.hub.execute(&effect).await.unwrap_or(Ok(Vec::new()))
                }
                effect => self.inner.execute(effect).await,
            }
        })
    }

    fn get_blob<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Option<Vec<u8>>, SyncError>> {
        self.inner.get_blob(id)
    }
}

/// Deterministic seeds that count from a start, shared by a session and its server.
#[derive(Debug)]
struct Seeds(std::sync::atomic::AtomicU64);

impl SeedSource for Seeds {
    fn next_seed(&self) -> u128 {
        u128::from(self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst)) << 64
    }
}

/// A dail server on `127.0.0.1` over a [`MemoryBackend`].
pub struct TestServer {
    dir: PathBuf,
    backend: Arc<MemoryBackend>,
    hub: Arc<ServeHub>,
    session: Option<Session>,
    server: Option<DailServer>,
    seeds: Arc<Seeds>,
    addr: SocketAddr,
    fingerprint: String,
}

impl std::fmt::Debug for TestServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestServer")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

/// A hub over `<dir>/config.toml`, as `dail serve` makes one when it starts.
fn new_hub(dir: &Path) -> Result<Arc<ServeHub>, SyncError> {
    let config = Arc::new(ConfigStore::open(dir.join("config.toml"))?);
    Ok(Arc::new(ServeHub::new(config, Arc::new(SystemClock))))
}

impl TestServer {
    /// Starts a server in `dir` (its `config.toml` and `serve/`) over `backend`, on a free
    /// loopback port.
    pub async fn start(dir: &Path, backend: MemoryBackend) -> Result<Self, SyncError> {
        std::fs::create_dir_all(dir)
            .map_err(|err| SyncError::Serve(format!("{}: {err}", dir.display())))?;
        let mut server = Self {
            dir: dir.to_path_buf(),
            backend: Arc::new(backend),
            hub: new_hub(dir)?,
            session: None,
            server: None,
            // Far from the devices' seeds, so that IDs never collide by accident.
            seeds: Arc::new(Seeds(std::sync::atomic::AtomicU64::new(1 << 40))),
            addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            fingerprint: String::new(),
        };
        server.run().await?;
        Ok(server)
    }

    /// Opens the session and listens (on the same port again after [`TestServer::restart`]).
    async fn run(&mut self) -> Result<(), SyncError> {
        let backend = Arc::new(Served {
            inner: Arc::clone(&self.backend),
            hub: Arc::clone(&self.hub),
        });
        let session = Session::open(
            backend,
            Arc::new(SystemClock),
            Arc::clone(&self.seeds) as Arc<dyn SeedSource>,
        )
        .await?;
        self.session = Some(session);
        self.listen().await
    }

    /// Serves the session on the port (the same one again after a stop).
    async fn listen(&mut self) -> Result<(), SyncError> {
        let session = self.session()?.client();
        let identity =
            ServerIdentity::load_or_create(&self.dir.join("serve"), &["127.0.0.1".to_owned()])?;
        let mut server = DailServer::start(
            &identity,
            Arc::clone(&self.hub),
            session,
            Arc::clone(&self.seeds) as Arc<dyn SeedSource>,
            ServeOptions::new(self.dir.join("serve").join("uploads")),
        )?;
        // The port of a server that just stopped may take a moment to be free again.
        let mut tries = 0;
        let bound = loop {
            match server.listen(self.addr).await {
                Ok(bound) => break bound,
                Err(_) if tries < 50 && self.addr.port() != 0 => {
                    tries += 1;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(err) => return Err(err),
            }
        };
        self.addr = bound;
        self.fingerprint = identity.fingerprint().to_owned();
        self.server = Some(server);
        Ok(())
    }

    /// Stops listening (every connection closes); the session keeps running, so the test can
    /// change the server's mail while its devices are offline.
    pub fn stop_listening(&mut self) {
        drop(self.server.take());
    }

    /// Listens again on the same port after [`TestServer::stop_listening`].
    pub async fn listen_again(&mut self) -> Result<(), SyncError> {
        self.listen().await
    }

    /// Stops listening (every connection closes) and shuts the session down, then starts both
    /// again on the same port: a new server process over the same mail, devices and
    /// `serve/` directory (the hub is new, so what it remembers comes from disk).
    pub async fn restart(&mut self) -> Result<(), SyncError> {
        drop(self.server.take());
        if let Some(session) = self.session.take() {
            session.shutdown().await?;
        }
        self.hub = new_hub(&self.dir)?;
        self.run().await
    }

    /// The directory of the server's profile (`config.toml`, `serve/`).
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The server's session.
    pub fn session(&self) -> Result<&Session, SyncError> {
        self.session.as_ref().ok_or(SyncError::SessionClosed)
    }

    /// The fake mail servers and the server's store.
    pub fn backend(&self) -> &MemoryBackend {
        &self.backend
    }

    /// Where it listens.
    pub const fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The SHA-256 fingerprint of its certificate.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Pairs a device with `role` (`start_pairing`, then the `pair` exchange over TLS) and
    /// returns its token.
    pub async fn pair(&self, role: DeviceRole) -> Result<Secret, SyncError> {
        let session = self.session()?;
        let receipt = session
            .submit(
                Command::StartPairing {
                    role,
                    name: "Test device".to_owned(),
                },
                Origin::Cli,
            )
            .await?
            .map_err(|err| SyncError::Serve(err.to_string()))?;
        session.flush().await?;
        let code = receipt.data["code"]
            .as_str()
            .ok_or_else(|| SyncError::Serve("start_pairing gave no code".to_owned()))?;
        let paired = app_sync::remote::pair(
            "127.0.0.1",
            self.addr.port(),
            &self.fingerprint,
            &Secret::new(code),
        )
        .await?;
        Ok(paired.token)
    }

    /// The state of a device paired with this server as its remote profile opens it
    /// (`[remote]`: the pinned server, `connecting`), and the keyring entry of its token.
    pub fn device_state(&self, token: SecretRef) -> AppState {
        AppState {
            link: Some(LinkState {
                status: LinkStatus::Connecting,
                server: Some(ServerEndpoint {
                    host: "127.0.0.1".to_owned(),
                    port: self.addr.port(),
                    offered: None,
                    pairing: None,
                    fingerprint: Some(self.fingerprint.clone()),
                    token: Some(token),
                }),
                ..LinkState::default()
            }),
            ..AppState::default()
        }
    }

    /// A device paired with `role` as a remote profile: its session (opened with
    /// [`Session::open_remote`] over its own [`MemoryBackend`], seeds counting from
    /// `seed_start`), its keyring and its backend (what it executed).
    pub async fn device(
        &self,
        role: DeviceRole,
        seed_start: u64,
        options: RemoteOptions,
    ) -> Result<TestDevice, SyncError> {
        let token = self.pair(role).await?;
        let entry = SecretRef::for_server(uuid::Uuid::from_u128(u128::from(seed_start)));
        let secrets = Arc::new(MemorySecrets::new());
        app_sync::SecretStore::set(secrets.as_ref(), &entry, &token)?;
        open_device(self.device_state(entry), secrets, seed_start, options).await
    }
}

/// A device of a [`TestServer`].
pub struct TestDevice {
    /// Its session.
    pub session: Session,
    /// Its keyring, with the device token.
    pub secrets: Arc<MemorySecrets>,
    /// Its own backend: the effects it ran, and its store.
    pub backend: Arc<MemoryBackend>,
}

impl std::fmt::Debug for TestDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestDevice").finish_non_exhaustive()
    }
}

impl TestDevice {
    /// Shuts the session down and opens a new one over the same backend and store, as dail
    /// does when it starts again; its seeds count from `seed_start`.
    pub async fn restart(self, seed_start: u64, options: RemoteOptions) -> Result<Self, SyncError> {
        let Self {
            session,
            secrets,
            backend,
        } = self;
        session.shutdown().await?;
        open_session(backend, secrets, seed_start, options).await
    }
}

/// Opens a device session from `state` (its persisted snapshot) with `secrets`, as
/// [`TestServer::device`] does.
pub async fn open_device(
    state: AppState,
    secrets: Arc<MemorySecrets>,
    seed_start: u64,
    options: RemoteOptions,
) -> Result<TestDevice, SyncError> {
    let backend = Arc::new(MemoryBackend::new().with_store(MemoryStore::with_state(state)));
    open_session(backend, secrets, seed_start, options).await
}

async fn open_session(
    backend: Arc<MemoryBackend>,
    secrets: Arc<MemorySecrets>,
    seed_start: u64,
    options: RemoteOptions,
) -> Result<TestDevice, SyncError> {
    let session = Session::open_remote(
        Arc::clone(&backend) as Arc<dyn Backend>,
        Arc::clone(&secrets) as Arc<dyn app_sync::SecretStore>,
        Arc::new(SystemClock) as Arc<dyn Clock>,
        Arc::new(Seeds(std::sync::atomic::AtomicU64::new(seed_start))),
        None,
        options,
    )
    .await?;
    Ok(TestDevice {
        session,
        secrets,
        backend,
    })
}
