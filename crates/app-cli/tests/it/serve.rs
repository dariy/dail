//! `dail serve` and `dail remote` (ADR 0017, SRV-4) on the memory backend over loopback: what
//! they refuse, a fingerprint that does not match, and a clean stop. The full pairing runs in
//! the smoke gate (`xtask/src/smoke.rs`).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-cli-serve-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn dail(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dail"))
        .args(args)
        .current_dir(root())
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A `dail serve` child on `profile`, killed when dropped.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts `dail serve --backend memory` on `profile` and waits for its `listening.json`.
fn serve(profile: &Path, log: &Path) -> (Server, Value) {
    let child = Command::new(env!("CARGO_BIN_EXE_dail"))
        .args(["--backend", "memory", "--profile-dir"])
        .arg(profile)
        .args(["serve", "--listen", "127.0.0.1:0"])
        .current_dir(root())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(log).unwrap())
        .spawn()
        .unwrap();
    let mut server = Server(child);
    let file = profile.join("serve").join("listening.json");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(listening) = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
        {
            return (server, listening);
        }
        if let Some(status) = server.0.try_wait().unwrap() {
            panic!(
                "dail serve exited with {status}: {}",
                std::fs::read_to_string(log).unwrap_or_default()
            );
        }
        assert!(Instant::now() < deadline, "dail serve did not listen");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn serve_refuses_what_it_cannot_do() {
    let dir = temp("refuse");
    let profile = dir.join("server");
    let profile_arg = profile.to_str().unwrap();

    // A private memory session can't be served or paired with.
    let out = dail(&["--backend", "memory", "serve"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(stderr(&out).contains("--profile-dir"), "{}", stderr(&out));

    // Nothing serves the profile yet.
    let on = |rest: &[&str]| {
        let mut args = vec!["--backend", "memory", "--profile-dir", profile_arg];
        args.extend_from_slice(rest);
        dail(&args)
    };
    let out = on(&["serve", "pair", "--role", "user", "--name", "Laptop"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(stderr(&out).contains("not running"), "{}", stderr(&out));

    let (server, listening) = serve(&profile, &dir.join("serve.log"));
    // One owner per profile.
    let out = on(&["serve", "--listen", "127.0.0.1:0"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(stderr(&out).contains("already running"), "{}", stderr(&out));
    // A revocation without --yes needs a terminal to ask on.
    let out = on(&["serve", "revoke", "00000000-0000-0000-0000-000000000001"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(stderr(&out).contains("--yes"), "{}", stderr(&out));
    // An unknown device is refused by the engine.
    let out = on(&[
        "serve",
        "revoke",
        "00000000-0000-0000-0000-000000000001",
        "--yes",
    ]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");

    // A device that is told another fingerprint does not pair and keeps no link.
    let address = listening["addresses"][0].as_str().unwrap();
    let out = dail(&[
        "--backend",
        "memory",
        "remote",
        "add",
        address,
        "--code",
        "ABCDEFGH",
        "--fingerprint",
        &"00".repeat(32),
    ]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(stderr(&out).contains("Do not pair"), "{}", stderr(&out));
    let last: Value = String::from_utf8_lossy(&out.stdout)
        .lines()
        .last()
        .map(|line| serde_json::from_str(line).unwrap())
        .unwrap();
    assert_eq!(last["name"], "remote");
    assert_eq!(last["ok"], false);
    assert!(last["link"].is_null(), "{last}");
    // Without a terminal, both answers must be given as flags.
    let out = dail(&["--backend", "memory", "remote", "add", address]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    // A profile with no link.
    let out = dail(&["--backend", "memory", "remote", "status"]);
    assert!(out.status.success(), "{out:?}");
    let status: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(status["linked"], false);
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `systemctl stop` sends SIGTERM: the server stops listening, saves and exits 0.
#[cfg(unix)]
#[test]
fn serve_stops_cleanly_on_sigterm() {
    let dir = temp("sigterm");
    let profile = dir.join("server");
    let log = dir.join("serve.log");
    let (mut server, _) = serve(&profile, &log);
    let pid = server.0.id().to_string();
    let killed = Command::new("kill").args(["-TERM", &pid]).status().unwrap();
    assert!(killed.success());
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = server.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "dail serve did not stop");
        std::thread::sleep(Duration::from_millis(50));
    };
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(status.success(), "{status}: {text}");
    assert!(text.contains("stopping"), "{text}");
    assert!(!profile.join("serve").join("listening.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A `listening.json` left by a server that was killed does not make another owner look like
/// `dail serve`: the owner removes it when it takes the lock, and `serve pair` refuses.
#[test]
fn a_stale_listening_file_does_not_fool_serve_pair() {
    let dir = temp("stale");
    let profile = dir.join("server");
    let profile_arg = profile.to_str().unwrap();
    std::fs::create_dir_all(profile.join("serve")).unwrap();
    let stale = profile.join("serve").join("listening.json");
    std::fs::write(
        &stale,
        r#"{"addresses":["127.0.0.1:7443"],"advertise":[],"fingerprint":"00","pid":1}"#,
    )
    .unwrap();
    // A long-running owner that is not `dail serve`: MCP over stdio, until its stdin closes.
    let mut owner = Server(
        Command::new(env!("CARGO_BIN_EXE_dail"))
            .args(["--backend", "memory", "--profile-dir", profile_arg, "mcp"])
            .current_dir(root())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let refusal = loop {
        let out = dail(&[
            "--backend",
            "memory",
            "--profile-dir",
            profile_arg,
            "serve",
            "pair",
            "--role",
            "user",
            "--name",
            "Laptop",
        ]);
        assert_ne!(out.status.code(), Some(0), "{out:?}");
        let text = stderr(&out);
        // Until the owner listens, nothing serves the profile.
        if !text.contains("not running") {
            break text;
        }
        assert!(Instant::now() < deadline, "the owner did not listen");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(refusal.contains("is not `dail serve`"), "{refusal}");
    assert!(!stale.exists(), "the owner removed the stale file");
    drop(owner.0.stdin.take());
    drop(owner);
    let _ = std::fs::remove_dir_all(&dir);
}
