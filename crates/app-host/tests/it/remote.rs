//! A phone as a device of a dail server (ADR 0017, SRV-7), against an in-process
//! `app_testkit::TestServer`: the onboarding's *Connect to a dail server* driven through frames,
//! the profile opened again as `BackendChoice::Remote`, the background check that talks to no
//! mail server, and a picked file uploaded to the server.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use app_core::{Command, DeviceRole, LinkStatus, Origin, ProviderPreset, Secret};
use app_host::{BackendChoice, Config, Host, HostError, Output, Platform, Task};
use app_mobile::UiEvent;
use app_mobile::event::Field;
use app_sync::SecretStore;
use app_testkit::{Fixture, MemoryBackend, MemorySecrets, TestServer};

/// The phone's secret store (Keystore, Keychain) in these tests: one for the process.
fn tokens() -> Arc<MemorySecrets> {
    static TOKENS: OnceLock<Arc<MemorySecrets>> = OnceLock::new();
    Arc::clone(TOKENS.get_or_init(|| Arc::new(MemorySecrets::new())))
}

fn phone_secrets() -> Result<Arc<dyn SecretStore>, HostError> {
    Ok(tokens())
}

fn no_logs(_: &Path) {}

const PHONE: Platform = Platform {
    secrets: phone_secrets,
    logging: no_logs,
    install: None,
};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-host-remote-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn config(dir: &Path, backend: BackendChoice) -> Config {
    Config {
        profile_dir: dir.to_path_buf(),
        cache_dir: None,
        outgoing_dir: Some(dir.join("outgoing")),
        backend,
        lang: None,
        version: None,
        ca_certs: Vec::new(),
    }
}

fn open(dir: &Path, backend: BackendChoice) -> Result<Host, HostError> {
    Host::open(
        &config(dir, backend),
        PHONE,
        Arc::new(|| {}),
        Arc::new(|_| {}),
    )
}

/// Frames until `done` holds.
fn frame_until(host: &Host, what: &str, done: impl Fn(&Output) -> bool) -> Output {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let out = host.frame();
        if done(&out) {
            return out;
        }
        assert!(Instant::now() < deadline, "no frame with {what}: {out:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn tap(host: &Host, out: &Output, label: &str) -> Output {
    let sheet = out.frame.setup.as_ref().unwrap();
    let action = sheet
        .actions
        .iter()
        .find(|b| b.label == label)
        .unwrap_or_else(|| panic!("no {label}: {sheet:?}"))
        .action;
    host.event(UiEvent::Tap { action })
}

/// A dail server with the fixture's account added and synced, on its own runtime.
fn server(dir: &Path) -> (tokio::runtime::Runtime, TestServer) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let server = runtime.block_on(async {
        let fixture = Fixture::builtin("basic").unwrap();
        let email = fixture.accounts[0].email.clone();
        let server = TestServer::start(dir, MemoryBackend::from_fixture(fixture))
            .await
            .unwrap();
        let session = server.session().unwrap();
        let add = Command::AddAccount {
            preset: ProviderPreset::Hostinger,
            email,
            display_name: None,
            password: Some(Secret::new("pw")),
            imap: None,
            smtp: None,
        };
        for cmd in [add, Command::SyncNow { account: None }] {
            session.submit(cmd, Origin::Cli).await.unwrap().unwrap();
            session.flush().await.unwrap();
        }
        server
    });
    (runtime, server)
}

/// A pairing code from the server, as `dail serve pair` shows it.
fn code(runtime: &tokio::runtime::Runtime, server: &TestServer) -> String {
    runtime.block_on(async {
        let receipt = server
            .session()
            .unwrap()
            .submit(
                Command::StartPairing {
                    role: DeviceRole::User,
                    name: "Phone".to_owned(),
                },
                Origin::Cli,
            )
            .await
            .unwrap()
            .unwrap();
        server.session().unwrap().flush().await.unwrap();
        receipt.data["code"].as_str().unwrap().to_owned()
    })
}

#[test]
fn a_phone_pairs_from_the_first_run_then_opens_as_a_remote_profile_and_uploads() {
    let (runtime, server) = server(&temp("server"));
    let dir = temp("phone");

    // First run: a plain profile, the setup offers the server.
    let host = open(&dir, BackendChoice::Live).unwrap();
    assert_eq!(host.backend(), &BackendChoice::Live);
    host.event(UiEvent::Configure {
        width_dp: 360,
        height_dp: 800,
        dark: false,
        font_scale: 1.0,
        locale: Some("en-US".to_owned()),
        keyboard: false,
    });
    let out = host.frame();
    let sheet = out.frame.setup.as_ref().unwrap();
    let choice = sheet.presets.last().unwrap();
    assert_eq!(choice.label, "Connect to a dail server");
    host.event(UiEvent::Tap {
        action: choice.action,
    });
    let out = tap(&host, &host.frame(), "Next");
    assert_eq!(
        out.frame.setup.as_ref().unwrap().fields[0].field,
        Field::Server
    );
    host.event(UiEvent::Text {
        field: Field::Server,
        text: format!("127.0.0.1:{}", server.addr().port()),
    });
    tap(&host, &host.frame(), "Connect");
    let out = frame_until(&host, "the offered fingerprint", |out| {
        out.frame
            .setup
            .as_ref()
            .is_some_and(|s| !s.fingerprints.is_empty())
    });
    let shown: String = out.frame.setup.as_ref().unwrap().fingerprints[0]
        .lines
        .concat()
        .replace(' ', "");
    assert_eq!(shown, server.fingerprint(), "the server's own certificate");
    host.event(UiEvent::Text {
        field: Field::Code,
        text: code(&runtime, &server),
    });
    tap(&host, &host.frame(), "Pair");
    let out = frame_until(&host, "paired", |out| {
        out.frame.setup.as_ref().is_some_and(|s| {
            s.note
                .as_deref()
                .is_some_and(|n| n.starts_with("Paired with"))
        })
    });
    tap(&host, &out, "Close");
    // The mirror fills from the server.
    frame_until(&host, "the server's account", |_| {
        !host.state().accounts.is_empty()
            && host.state().link.as_ref().is_some_and(|l| l.is_online())
    });
    let token = host
        .state()
        .link
        .as_ref()
        .unwrap()
        .server
        .as_ref()
        .unwrap()
        .token
        .clone()
        .unwrap();
    assert!(
        tokens().get(&token).unwrap().is_some(),
        "the token is in the phone's store"
    );
    assert!(
        !std::fs::read_to_string(dir.join("config.toml"))
            .unwrap()
            .contains(&tokens().get(&token).unwrap().unwrap().expose().to_owned()),
        "never the token itself"
    );
    host.close();
    drop(host);

    // Opened again: `[remote]` makes it a remote profile.
    let host = open(&dir, BackendChoice::Live).unwrap();
    assert_eq!(host.backend(), &BackendChoice::Remote);
    frame_until(&host, "online", |_| {
        host.state()
            .link
            .as_ref()
            .is_some_and(|l| l.status == LinkStatus::Online)
    });
    // The background check refreshes the link; it asks no mail server for anything.
    assert!(
        host.background(Task::Check, Duration::from_secs(20))
            .unwrap()
    );
    let out = host.frame();
    assert!(
        out.frame
            .top_bar
            .subtitle
            .as_deref()
            .is_some_and(|s| s.starts_with("Online")),
        "{:?}",
        out.frame.top_bar.subtitle
    );

    // A picked file: copied into the app, then uploaded with `attach_file`.
    host.event(UiEvent::Tap {
        action: out.frame.fab.unwrap().action,
    });
    let draft = frame_until(&host, "the composer", |out| out.frame.composer.is_some())
        .frame
        .composer
        .unwrap()
        .draft;
    let picked = temp("picked");
    std::fs::create_dir_all(&picked).unwrap();
    std::fs::write(picked.join("fd"), b"hello server").unwrap();
    let copy = host.import(draft, "notes.txt", &picked.join("fd")).unwrap();
    assert!(copy.starts_with(dir.join("outgoing")));
    let deadline = Instant::now() + Duration::from_secs(20);
    let on_server = loop {
        let state = server.session().unwrap().state();
        if let Some(file) = state.drafts.get(&draft).and_then(|d| d.attachments.first()) {
            break file.clone();
        }
        assert!(
            Instant::now() < deadline,
            "the upload never reached the server"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(on_server.name, "notes.txt");
    assert_ne!(Path::new(&on_server.path), copy, "the server's own copy");
    assert_eq!(std::fs::read(&on_server.path).unwrap(), b"hello server");
    host.close();
    drop(server);
    drop(runtime);
}

#[test]
fn remote_is_refused_for_a_profile_without_a_server() {
    let dir = temp("plain");
    let err = open(&dir, BackendChoice::Remote).unwrap_err();
    assert!(matches!(err, HostError::NotRemote(_)), "{err}");
    assert!(err.to_string().starts_with("ERR311: "));
    let config: Config =
        serde_json::from_str(r#"{"profile_dir": "/p", "backend": "remote"}"#).unwrap();
    assert_eq!(config.backend, BackendChoice::Remote);
}
