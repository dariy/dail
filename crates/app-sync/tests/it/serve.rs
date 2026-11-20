//! The dail server (ADR 0017) over TLS on loopback: the certificate, the pinned dialer and the
//! probe, pairing, `hello`, roles, idempotent and seeded `submit`, `watch`, `blob`, `put_file`
//! and revocation.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use app_core::{
    AccountId, AppState, Command, DeviceId, DeviceRole, Effect, Event, Origin, PAIRING_TTL_MS,
    ProviderPreset, RequestId, Secret, SettingKey,
};
use app_io::ConfigStore;
use app_sync::remote::{self, RemoteChange, RemoteClient, RemoteTarget};
use app_sync::rpc::CallError;
use app_sync::serve::{
    self, DailServer, MAX_UPLOAD_BYTES, MAX_WRONG_CODES, ServeHub, ServeOptions, ServerIdentity,
};
use app_sync::{Backend, BoxFuture, Clock, CounterSeeds, Session, SyncError};
use serde_json::json;

/// A clock the test moves by hand.
#[derive(Debug)]
struct ManualClock(AtomicI64);

impl ManualClock {
    fn advance(&self, ms: i64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Runs the server's effects on the hub, serves blobs from a map, and acknowledges the rest.
struct ServingBackend {
    hub: Arc<ServeHub>,
    blobs: HashMap<String, Vec<u8>>,
}

impl Backend for ServingBackend {
    fn name(&self) -> &'static str {
        "serving"
    }

    fn load_state(&self) -> BoxFuture<'_, Result<Option<AppState>, SyncError>> {
        Box::pin(async { Ok(None) })
    }

    fn execute(&self, effect: Effect) -> BoxFuture<'_, Result<Vec<Event>, SyncError>> {
        Box::pin(async move {
            match self.hub.execute(&effect).await {
                Some(result) => result,
                None => Ok(vec![Event::Persisted {}]),
            }
        })
    }

    fn get_blob<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<Option<Vec<u8>>, SyncError>> {
        Box::pin(async move { Ok(self.blobs.get(id).cloned()) })
    }
}

const BLOB: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Fixture {
    dir: PathBuf,
    clock: Arc<ManualClock>,
    hub: Arc<ServeHub>,
    session: Session,
    server: DailServer,
    addr: SocketAddr,
    fingerprint: String,
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-serve-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn serve(name: &str, allow_send: bool) -> Fixture {
    let dir = temp_dir(name);
    let clock = Arc::new(ManualClock(AtomicI64::new(1_700_000_000_000)));
    let config = Arc::new(ConfigStore::open(dir.join("config.toml")).unwrap());
    let hub = Arc::new(ServeHub::new(config, Arc::clone(&clock) as Arc<dyn Clock>));
    let backend = Arc::new(ServingBackend {
        hub: Arc::clone(&hub),
        blobs: HashMap::from([(BLOB.to_owned(), b"From: a\r\n\r\nbody".to_vec())]),
    });
    let seeds = Arc::new(CounterSeeds::starting_at(1));
    let session = Session::open(backend, Arc::clone(&clock) as Arc<dyn Clock>, seeds.clone())
        .await
        .unwrap();
    let identity =
        ServerIdentity::load_or_create(&dir.join("serve"), &["127.0.0.1".to_owned()]).unwrap();
    let mut options = ServeOptions::new(dir.join("serve").join("uploads"));
    options.allow_send = allow_send;
    let mut server = DailServer::start(
        &identity,
        Arc::clone(&hub),
        session.client(),
        seeds,
        options,
    )
    .unwrap();
    let addr = server.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    Fixture {
        dir,
        clock,
        hub,
        session,
        server,
        addr,
        fingerprint: identity.fingerprint().to_owned(),
    }
}

impl Fixture {
    /// A code of `start_pairing`, open on the hub.
    async fn code(&self, role: DeviceRole) -> Secret {
        let receipt = self
            .session
            .submit(
                Command::StartPairing {
                    role,
                    name: "Phone".to_owned(),
                },
                Origin::Cli,
            )
            .await
            .unwrap()
            .unwrap();
        self.session.flush().await.unwrap();
        Secret::new(receipt.data["code"].as_str().unwrap())
    }

    async fn pair(&self, code: &Secret) -> Result<remote::Paired, SyncError> {
        remote::pair("127.0.0.1", self.addr.port(), &self.fingerprint, code).await
    }

    fn target(&self, token: &Secret) -> RemoteTarget {
        RemoteTarget {
            host: "127.0.0.1".to_owned(),
            port: self.addr.port(),
            fingerprint: self.fingerprint.clone(),
            token: token.clone(),
        }
    }

    /// A paired device's client.
    async fn device(&self, role: DeviceRole) -> (RemoteClient, DeviceId) {
        let paired = self.pair(&self.code(role).await).await.unwrap();
        let client = RemoteClient::connect(self.target(&paired.token))
            .await
            .unwrap();
        (client, paired.device)
    }

    async fn raw(&self) -> app_sync::rpc::RpcClient {
        remote::open_rpc("127.0.0.1", self.addr.port(), &self.fingerprint)
            .await
            .unwrap()
    }
}

fn poll_secs(value: u64) -> Command {
    Command::SetSetting {
        key: SettingKey::PollSecs,
        value: json!(value),
    }
}

async fn next(watch: &mut remote::RemoteWatch) -> Option<RemoteChange> {
    tokio::time::timeout(Duration::from_secs(5), watch.next())
        .await
        .unwrap()
}

fn request_id(n: u128) -> RequestId {
    RequestId::from_uuid(uuid::Uuid::from_u128(n))
}

#[tokio::test]
async fn the_certificate_is_made_once_and_kept() {
    let dir = temp_dir("cert").join("serve");
    let first = ServerIdentity::load_or_create(&dir, &["home.example.org".to_owned()]).unwrap();
    let again = ServerIdentity::load_or_create(&dir, &["other.example.org".to_owned()]).unwrap();
    assert_eq!(first.fingerprint(), again.fingerprint());
    assert_eq!(first.fingerprint().len(), 64);
    assert_eq!(
        app_sync::trust::fingerprint(first.certificate()),
        first.fingerprint()
    );
    let info = app_sync::trust::describe(first.certificate());
    assert_eq!(info.subject, "CN=dail server");
    assert_eq!(info.issuer, info.subject);
    let years = (info.not_after_ms - info.not_before_ms) / (365 * 86_400_000);
    assert_eq!(years, 20);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = |path: PathBuf| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(dir.clone()), 0o700);
        assert_eq!(mode(dir.join("key.p256")), 0o600);
        assert_eq!(mode(dir.join("cert.der")), 0o600);
    }
    // A lost certificate is made again for the same key, with a new fingerprint.
    std::fs::remove_file(dir.join("cert.der")).unwrap();
    let remade = ServerIdentity::load_or_create(&dir, &[]).unwrap();
    assert_ne!(remade.fingerprint(), first.fingerprint());
    let key = std::fs::read(dir.join("key.p256")).unwrap();
    assert_eq!(key.len(), 32);
}

#[tokio::test]
async fn the_dialer_accepts_only_the_pinned_certificate_and_the_probe_reads_it() {
    let fx = serve("pin", false).await;
    let port = fx.addr.port();
    assert_eq!(
        remote::probe("127.0.0.1", port).await.unwrap(),
        fx.fingerprint
    );
    assert!(
        remote::open_rpc("127.0.0.1", port, &fx.fingerprint)
            .await
            .is_ok()
    );
    let other = "00".repeat(32);
    let err = remote::open_rpc("127.0.0.1", port, &other)
        .await
        .unwrap_err();
    assert!(
        matches!(err, SyncError::Remote(ref m) if m.contains("TLS")),
        "{err}"
    );
    // The pinned fingerprint is compared without regard to case.
    assert!(
        remote::open_rpc("127.0.0.1", port, &fx.fingerprint.to_uppercase())
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn a_code_pairs_a_device_once() {
    let fx = serve("pair", false).await;
    let code = fx.code(DeviceRole::User).await;
    assert_eq!(fx.hub.open_codes(), 1);
    // Case and spaces do not matter.
    let typed = Secret::new(format!(" {} ", code.expose().to_lowercase()));
    let paired = fx.pair(&typed).await.unwrap();
    assert_eq!(paired.role, DeviceRole::User);
    assert_eq!(fx.hub.open_codes(), 0);
    // The token is 32 bytes in base64url.
    assert_eq!(paired.token.expose().len(), 43);

    // The device is in the state and in config.toml, with the token's hash only.
    fx.session.flush().await.unwrap();
    let device = fx.session.state().devices[&paired.device].clone();
    assert_eq!(device.name, "Phone");
    assert_eq!(device.paired_at_ms, 1_700_000_000_000);
    let config = std::fs::read_to_string(fx.dir.join("config.toml")).unwrap();
    assert!(config.contains("[[serve.devices]]"), "{config}");
    assert!(config.contains("token_sha256"));
    assert!(!config.contains(paired.token.expose()));
    let reopened = app_io::Config::load(&fx.dir.join("config.toml"))
        .unwrap()
        .unwrap();
    assert_eq!(reopened.serve.devices.len(), 1);
    let mut state = AppState::default();
    reopened.apply_to(&mut state);
    assert!(state.devices.contains_key(&paired.device));

    // Single use.
    let again = fx.pair(&code).await.unwrap_err();
    assert!(
        matches!(again, SyncError::Remote(ref m) if m.contains("refused")),
        "{again}"
    );

    // hello with the token works, and the device is seen.
    let client = RemoteClient::connect(fx.target(&paired.token))
        .await
        .unwrap();
    assert_eq!(client.hello().device.id, paired.device);
    assert_eq!(client.hello().protocol, serve::PROTOCOL);
    assert_eq!(client.hello().instance, fx.server.instance());
    fx.session.flush().await.unwrap();
    assert_eq!(
        fx.session.state().devices[&paired.device].last_seen_ms,
        Some(1_700_000_000_000)
    );
}

#[tokio::test]
async fn wrong_and_expired_codes_are_refused() {
    let fx = serve("expired", false).await;
    let code = fx.code(DeviceRole::User).await;
    assert!(fx.pair(&Secret::new("AAAAAAAA")).await.is_err());
    fx.clock.advance(PAIRING_TTL_MS + 1);
    assert!(fx.pair(&code).await.is_err());
    assert_eq!(fx.hub.open_codes(), 0);
}

#[tokio::test]
async fn five_wrong_codes_in_a_minute_burn_every_code() {
    let fx = serve("burn", false).await;
    let code = fx.code(DeviceRole::User).await;
    let other = fx.code(DeviceRole::Agent).await;
    for attempt in 1..MAX_WRONG_CODES {
        assert!(fx.pair(&Secret::new("AAAAAAAA")).await.is_err());
        fx.clock.advance(1000);
        assert_eq!(fx.hub.open_codes(), 2, "attempt {attempt}");
    }
    assert!(fx.pair(&Secret::new("AAAAAAAA")).await.is_err());
    assert_eq!(fx.hub.open_codes(), 0);
    assert!(fx.pair(&code).await.is_err());
    assert!(fx.pair(&other).await.is_err());

    // Wrong codes spread over more than a minute burn nothing.
    // (The two refusals above count too: a minute later they no longer do.)
    fx.clock.advance(60_000);
    let code = fx.code(DeviceRole::User).await;
    for _ in 0..MAX_WRONG_CODES {
        assert!(fx.pair(&Secret::new("AAAAAAAA")).await.is_err());
        fx.clock.advance(20_000);
    }
    let paired = fx.pair(&code).await;
    assert!(paired.is_ok(), "{paired:?}");
}

#[tokio::test]
async fn a_wrong_token_a_wrong_protocol_or_another_first_request_close_the_connection() {
    let fx = serve("hello", false).await;
    let (_, _) = fx.device(DeviceRole::User).await;

    let rpc = fx.raw().await;
    let token = "A".repeat(43);
    let err = rpc
        .call(
            "hello",
            json!({ "token": token, "protocol": 1, "client_version": "x" }),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            CallError::Refused {
                code: serve::NOT_AUTHORIZED,
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(
        rpc.call("state", json!(null)).await.unwrap_err(),
        CallError::Closed
    );

    let paired = fx.pair(&fx.code(DeviceRole::User).await).await.unwrap();
    let rpc = fx.raw().await;
    let err = rpc
        .call(
            "hello",
            json!({ "token": paired.token.expose(), "protocol": 2, "client_version": "x" }),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            CallError::Refused {
                code: serve::PROTOCOL_MISMATCH,
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(
        rpc.call("state", json!(null)).await.unwrap_err(),
        CallError::Closed
    );

    let rpc = fx.raw().await;
    let err = rpc.call("state", json!(null)).await.unwrap_err();
    assert!(
        matches!(
            err,
            CallError::Refused {
                code: serve::NOT_AUTHORIZED,
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(
        rpc.call("state", json!(null)).await.unwrap_err(),
        CallError::Closed
    );
}

#[tokio::test]
async fn devices_submit_only_as_their_role_allows() {
    let fx = serve("roles", false).await;
    let (agent, _) = fx.device(DeviceRole::Agent).await;
    for origin in [
        Origin::Gui,
        Origin::Cli,
        Origin::McpAllowSend,
        Origin::Runtime,
    ] {
        let err = agent
            .submit(&poll_secs(300), origin, request_id(1), 1)
            .await
            .unwrap_err();
        assert!(
            matches!(err, SyncError::Remote(ref m) if m.contains("can't submit")),
            "{err}"
        );
    }
    assert_eq!(fx.session.state().settings.poll_secs, 120);
    let receipt = agent
        .submit(&Command::GetSettings {}, Origin::Mcp, request_id(2), 2)
        .await
        .unwrap();
    assert!(receipt.is_ok(), "{receipt:?}");

    let (user, _) = fx.device(DeviceRole::User).await;
    assert!(
        user.submit(&poll_secs(300), Origin::Mcp, request_id(3), 3)
            .await
            .is_err()
    );
    let receipt = user
        .submit(&poll_secs(300), Origin::Gui, request_id(4), 4)
        .await
        .unwrap();
    assert!(receipt.is_ok(), "{receipt:?}");
    assert_eq!(fx.session.state().settings.poll_secs, 300);

    // With --allow-send an agent may claim mcp_allow_send.
    let fx = serve("roles-send", true).await;
    let (agent, _) = fx.device(DeviceRole::Agent).await;
    let receipt = agent
        .submit(
            &Command::GetSettings {},
            Origin::McpAllowSend,
            request_id(5),
            5,
        )
        .await
        .unwrap();
    assert!(receipt.is_ok(), "{receipt:?}");
}

fn add_account() -> Command {
    Command::AddAccount {
        preset: ProviderPreset::Hostinger,
        email: "me@example.org".to_owned(),
        display_name: None,
        password: Some(Secret::new("pw")),
        imap: None,
        smtp: None,
    }
}

#[tokio::test]
async fn a_resent_submission_is_answered_from_memory_with_the_device_seed() {
    let fx = serve("idempotent", false).await;
    let paired = fx.pair(&fx.code(DeviceRole::User).await).await.unwrap();
    let user = RemoteClient::connect(fx.target(&paired.token))
        .await
        .unwrap();
    let seed: u128 = 0xabcd_0000_0000_0000_0000_0000_0000_0001;
    let first = user
        .submit(&add_account(), Origin::Cli, request_id(7), seed)
        .await
        .unwrap()
        .unwrap();
    // The ID derives from the device's seed, not the server's.
    let id = AccountId::from_uuid(uuid::Uuid::from_u128(seed));
    assert!(fx.session.state().accounts.contains_key(&id));
    // A resend on a new connection (after a dropped one): the same answer, and no second
    // dispatch, which would be refused as a duplicate address.
    drop(user);
    let user = RemoteClient::connect(fx.target(&paired.token))
        .await
        .unwrap();
    let again = user
        .submit(&add_account(), Origin::Cli, request_id(7), seed)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again, first);
    assert_eq!(fx.session.state().accounts.len(), 1);
    // A new request ID dispatches again.
    let other = user
        .submit(&add_account(), Origin::Cli, request_id(8), seed + 1)
        .await
        .unwrap();
    assert!(other.is_err(), "{other:?}");
}

#[tokio::test]
async fn watch_sends_diffs_and_resyncs() {
    let fx = serve("watch", false).await;
    let (user, _) = fx.device(DeviceRole::User).await;
    // Without a revision: a snapshot first.
    let mut watch = user.watch(None).await.unwrap();
    let Some(RemoteChange::Resync { .. }) = next(&mut watch).await else {
        panic!("a resync first");
    };
    user.submit(&poll_secs(300), Origin::Gui, request_id(1), 1)
        .await
        .unwrap()
        .unwrap();
    let Some(RemoteChange::Changed { revision, diff }) = next(&mut watch).await else {
        panic!("a change");
    };
    assert_eq!(diff.settings.as_ref().unwrap().poll_secs, 300);
    assert!(diff.accounts.is_empty());
    drop(watch);

    // Since a known revision: the change since then, without a snapshot.
    user.submit(&poll_secs(600), Origin::Gui, request_id(2), 2)
        .await
        .unwrap()
        .unwrap();
    let mut watch = user.watch(Some(revision)).await.unwrap();
    let Some(RemoteChange::Changed { diff, .. }) = next(&mut watch).await else {
        panic!("the change since the revision");
    };
    assert_eq!(diff.settings.as_ref().unwrap().poll_secs, 600);
    drop(watch);

    // Since an unknown revision: a snapshot.
    let mut watch = user.watch(Some(u64::MAX)).await.unwrap();
    let Some(RemoteChange::Resync { state, .. }) = next(&mut watch).await else {
        panic!("a resync");
    };
    assert_eq!(state.settings.as_ref().unwrap().poll_secs, 600);

    // `state` is a snapshot with its revision.
    let (revision, snapshot) = user.state().await.unwrap();
    assert!(revision > 0);
    assert_eq!(snapshot.settings.unwrap().poll_secs, 600);
}

#[tokio::test]
async fn blobs_and_uploads() {
    let fx = serve("files", false).await;
    let (user, _) = fx.device(DeviceRole::User).await;
    assert_eq!(
        user.blob(BLOB).await.unwrap().as_deref(),
        Some(&b"From: a\r\n\r\nbody"[..])
    );
    assert_eq!(user.blob(&"f".repeat(64)).await.unwrap(), None);
    assert!(user.blob("../../etc/passwd").await.is_err());

    let path = user
        .put_file("../../.evil\u{202e}fdp.exe", b"hello")
        .await
        .unwrap();
    let path = PathBuf::from(path);
    assert!(
        path.starts_with(fx.dir.join("serve").join("uploads")),
        "{}",
        path.display()
    );
    assert_eq!(path.file_name().unwrap(), "evilfdp.exe");
    assert_eq!(std::fs::read(&path).unwrap(), b"hello");
    let second = PathBuf::from(user.put_file("a.txt", b"1").await.unwrap());
    assert_ne!(second.parent(), path.parent());

    let too_big = vec![0u8; MAX_UPLOAD_BYTES + 1];
    let err = user.put_file("big.bin", &too_big).await.unwrap_err();
    assert!(
        matches!(err, SyncError::Remote(ref m) if m.contains("larger")),
        "{err}"
    );
    // The connection survives a refused upload.
    assert!(user.flush().await.is_ok());
}

#[tokio::test]
async fn revoking_a_device_closes_its_connections_at_once() {
    let fx = serve("revoke", false).await;
    let paired = fx.pair(&fx.code(DeviceRole::User).await).await.unwrap();
    let client = RemoteClient::connect(fx.target(&paired.token))
        .await
        .unwrap();
    let mut watch = client.watch(None).await.unwrap();
    assert!(matches!(
        watch.next().await,
        Some(RemoteChange::Resync { .. })
    ));
    assert_eq!(fx.hub.connections(), 2);
    fx.session
        .submit(
            Command::RevokeDevice {
                id: paired.device,
                confirm: true,
            },
            Origin::Cli,
        )
        .await
        .unwrap()
        .unwrap();
    fx.session.flush().await.unwrap();
    // The watch may still carry the device's removal; then it ends.
    for _ in 0..3 {
        if next(&mut watch).await.is_none() {
            break;
        }
    }
    assert_eq!(next(&mut watch).await, None);
    assert!(client.flush().await.is_err());
    assert!(
        RemoteClient::connect(fx.target(&paired.token))
            .await
            .is_err()
    );
    assert!(fx.hub.devices().is_empty());
    let config = app_io::Config::load(&fx.dir.join("config.toml"))
        .unwrap()
        .unwrap();
    assert!(config.serve.devices.is_empty());
}

#[tokio::test]
async fn a_restarted_hub_knows_its_devices() {
    let fx = serve("restart", false).await;
    let paired = fx.pair(&fx.code(DeviceRole::Agent).await).await.unwrap();
    let config = Arc::new(ConfigStore::open(fx.dir.join("config.toml")).unwrap());
    let hub = ServeHub::new(config, Arc::clone(&fx.clock) as Arc<dyn Clock>);
    let devices = hub.devices();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].id, paired.device);
    assert_eq!(devices[0].role, DeviceRole::Agent);
}

#[derive(Default)]
struct MemorySecrets(std::sync::Mutex<HashMap<app_core::SecretRef, Secret>>);

impl app_sync::SecretStore for MemorySecrets {
    fn set(&self, secret: &app_core::SecretRef, value: &Secret) -> Result<(), SyncError> {
        self.0.lock().unwrap().insert(secret.clone(), value.clone());
        Ok(())
    }

    fn get(&self, secret: &app_core::SecretRef) -> Result<Option<Secret>, SyncError> {
        Ok(self.0.lock().unwrap().get(secret).cloned())
    }

    fn delete(&self, secret: &app_core::SecretRef) -> Result<(), SyncError> {
        self.0.lock().unwrap().remove(secret);
        Ok(())
    }
}

struct NoVerifier;

impl app_sync::AccountVerifier for NoVerifier {
    fn verify<'a>(
        &'a self,
        _login: &'a str,
        _password: &'a Secret,
        _imap: &'a app_core::Endpoint,
        _smtp: &'a app_core::Endpoint,
        _trusted: &'a [String],
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async { Err("not used".to_owned()) })
    }
}

fn live(dir: &std::path::Path, secrets: Arc<MemorySecrets>) -> app_sync::LiveBackend {
    app_sync::LiveBackend::new(
        Arc::new(ConfigStore::open(dir.join("device.toml")).unwrap()),
        secrets,
        Arc::new(NoVerifier),
        Arc::new(crate::support::FakeImap::default()),
    )
}

#[tokio::test]
async fn the_live_backend_probes_and_pairs_as_a_device() {
    let fx = serve("live-device", false).await;
    let secrets = Arc::new(MemorySecrets::default());
    let backend = live(&fx.dir, Arc::clone(&secrets));
    let port = fx.addr.port();
    let events = backend
        .execute(Effect::ProbeServer {
            host: "127.0.0.1".to_owned(),
            port,
        })
        .await
        .unwrap();
    assert_eq!(
        events,
        [Event::ServerOffered {
            host: "127.0.0.1".to_owned(),
            port,
            fingerprint: fx.fingerprint.clone(),
        }]
    );

    let secret = app_core::SecretRef::for_server(uuid::Uuid::from_u128(9));
    let pair = |fingerprint: String, code: Secret| Effect::PairWithServer {
        host: "127.0.0.1".to_owned(),
        port,
        fingerprint,
        code,
        secret: secret.clone(),
    };
    let code = fx.code(DeviceRole::User).await;
    // Another fingerprint: refused before the code is sent, so the code stays open.
    let err = backend
        .execute(pair("ab".repeat(32), code.clone()))
        .await
        .unwrap_err();
    assert!(matches!(err, SyncError::Remote(_)), "{err}");
    assert_eq!(fx.hub.open_codes(), 1);
    let events = backend
        .execute(pair(fx.fingerprint.clone(), code))
        .await
        .unwrap();
    assert_eq!(
        events,
        [Event::ServerPaired {
            fingerprint: fx.fingerprint.clone(),
        }]
    );
    let token = secrets.0.lock().unwrap().get(&secret).cloned().unwrap();
    assert!(RemoteClient::connect(fx.target(&token)).await.is_ok());
    // Unreachable: a failure (`link_failed` through the executor).
    drop(fx);
    assert!(
        backend
            .execute(Effect::ProbeServer {
                host: "127.0.0.1".to_owned(),
                port,
            })
            .await
            .is_err()
    );
}

#[tokio::test]
async fn only_a_serving_live_backend_opens_codes() {
    let fx = serve("live-server", false).await;
    let open = Effect::OpenPairing {
        code: Secret::new("ABCDEFGH"),
        role: DeviceRole::User,
        name: "Laptop".to_owned(),
        expires_at_ms: i64::MAX,
    };
    let plain = live(&fx.dir, Arc::default());
    let err = plain.execute(open.clone()).await.unwrap_err();
    assert!(matches!(err, SyncError::Serve(_)), "{err}");
    let serving = live(&fx.dir, Arc::default()).with_serve(Arc::clone(&fx.hub));
    assert!(serving.execute(open).await.unwrap().is_empty());
    assert_eq!(fx.hub.open_codes(), 1);
    assert!(fx.pair(&Secret::new("ABCDEFGH")).await.is_ok());
}
