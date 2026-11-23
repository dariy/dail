//! The second step of the `smoke` gate (AGENTS.md §6, step 5): `dail serve --backend memory` on
//! loopback, driven only through the `dail` binary, as a user would. No network beyond
//! 127.0.0.1, no OS keyring, the same on Linux and Windows.
//!
//! 1. `dail serve --listen 127.0.0.1:0` starts on a fresh profile directory and describes itself
//!    in `serve/listening.json`;
//! 2. `dail serve pair` opens a code through the profile socket;
//! 3. `dail remote add` pairs a private memory profile over TLS with that code and the printed
//!    fingerprint;
//! 4. `dail batch tests/smoke/serve.jsonl` attaches to the server and sees the device;
//! 5. `dail serve revoke --yes` revokes it and `dail serve devices` lists none.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;

/// How long `dail serve` may take to listen.
const START_TIMEOUT: Duration = Duration::from_secs(30);

/// The `dail serve` child: killed and its profile removed when dropped, whatever happened.
struct Server {
    child: Child,
    dir: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Server {
    /// What `dail serve` wrote to stderr so far.
    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.join("serve.log")).unwrap_or_default()
    }
}

/// The `dail` binary cargo just built for the smoke gate's first step.
fn dail_binary(root: &Path, cargo: &str) -> Result<PathBuf> {
    let output = Command::new(cargo)
        .args(["build", "--quiet", "-p", "app-cli", "--message-format=json"])
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()
        .context("failed to spawn `cargo build -p app-cli`")?;
    if !output.status.success() {
        bail!("`cargo build -p app-cli` failed with {}", output.status);
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| message["target"]["name"] == "dail")
        .find_map(|message| message["executable"].as_str().map(PathBuf::from))
        .context("cargo built no `dail` executable")
}

/// Runs `dail <args>` to completion.
fn run(dail: &Path, root: &Path, args: &[&str]) -> Result<Output> {
    Command::new(dail)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("failed to spawn `dail {}`", args.join(" ")))
}

/// [`run_ok`] with the server profile's global options before `rest`.
fn run_on(dail: &Path, root: &Path, profile: &str, rest: &[&str]) -> Result<Vec<Value>> {
    let mut args = vec!["--backend", "memory", "--profile-dir", profile];
    args.extend_from_slice(rest);
    run_ok(dail, root, &args)
}

/// Runs `dail <args>`, which must succeed, and returns its stdout's JSON lines.
fn run_ok(dail: &Path, root: &Path, args: &[&str]) -> Result<Vec<Value>> {
    let output = run(dail, root, args)?;
    if !output.status.success() {
        bail!(
            "`dail {}` failed with {}:\n{}{}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| {
            serde_json::from_str(line)
                .with_context(|| format!("`dail {}` printed `{line}`", args.join(" ")))
        })
        .collect()
}

/// Starts `dail serve` on `profile` and waits until it listens: its `listening.json`.
fn start(dail: &Path, root: &Path, dir: PathBuf, profile: &str) -> Result<(Server, Value)> {
    std::fs::create_dir_all(&dir)?;
    let log = std::fs::File::create(dir.join("serve.log"))?;
    let child = Command::new(dail)
        .args([
            "--backend",
            "memory",
            "--profile-dir",
            profile,
            "serve",
            "--listen",
            "127.0.0.1:0",
        ])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .context("failed to spawn `dail serve`")?;
    let mut server = Server { child, dir };
    let file = Path::new(profile).join("serve").join("listening.json");
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        if let Some(listening) = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        {
            return Ok((server, listening));
        }
        if let Some(status) = server.child.try_wait()? {
            bail!("`dail serve` exited with {status}:\n{}", server.log());
        }
        if Instant::now() >= deadline {
            bail!(
                "`dail serve` did not listen within {START_TIMEOUT:?}:\n{}",
                server.log()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The smoke gate's `dail serve` step (see the module doc).
pub fn serve(root: &Path, cargo: &str) -> Result<()> {
    let started = Instant::now();
    let mut err = std::io::stderr().lock();
    writeln!(
        err,
        "smoke: dail serve --backend memory on 127.0.0.1, paired by dail remote add"
    )?;
    let dail = dail_binary(root, cargo)?;
    let dir = std::env::temp_dir().join(format!("dail-smoke-serve-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profile = dir.join("server").display().to_string();
    let (server, listening) = start(&dail, root, dir, &profile)?;
    let steps = drive(&dail, root, &profile, &listening);
    if let Err(err) = steps {
        return Err(err.context(format!("`dail serve` log:\n{}", server.log())));
    }
    drop(server);
    writeln!(
        err,
        "smoke: dail serve step passed in {:.1} s",
        started.elapsed().as_secs_f64()
    )?;
    Ok(())
}

fn drive(dail: &Path, root: &Path, profile: &str, listening: &Value) -> Result<()> {
    let address = listening["addresses"][0]
        .as_str()
        .context("listening.json names no address")?;
    let fingerprint = listening["fingerprint"]
        .as_str()
        .context("listening.json has no fingerprint")?;

    let pairing = run_on(
        dail,
        root,
        profile,
        &["serve", "pair", "--role", "user", "--name", "smoke laptop"],
    )?;
    let pairing = pairing
        .first()
        .context("`dail serve pair` printed nothing")?;
    let code = pairing["code"]
        .as_str()
        .context("`dail serve pair` printed no code")?;
    if pairing["fingerprint"] != fingerprint {
        bail!("`dail serve pair` shows another fingerprint than the server: {pairing}");
    }

    // A private memory profile (no --profile-dir) as the device.
    let added = run_ok(
        dail,
        root,
        &[
            "--backend",
            "memory",
            "remote",
            "add",
            address,
            "--code",
            code,
            "--fingerprint",
            fingerprint,
        ],
    )?;
    let last = added.last().context("`dail remote add` printed nothing")?;
    if last["ok"] != true || last["link"]["server"]["fingerprint"] != fingerprint {
        bail!("`dail remote add` did not pin the server: {last}");
    }

    let lines = run_on(dail, root, profile, &["batch", "tests/smoke/serve.jsonl"])?;
    let names: Vec<&str> = lines.iter().filter_map(|l| l["name"].as_str()).collect();
    if names != ["list_devices", "start_pairing", "get_settings"] {
        bail!("tests/smoke/serve.jsonl printed {names:?}");
    }
    let devices = lines[0]["data"]["devices"]
        .as_array()
        .context("list_devices has no devices")?;
    let [device] = devices.as_slice() else {
        bail!("expected the one paired device, got {devices:?}");
    };
    if device["name"] != "smoke laptop" || device["role"] != "user" {
        bail!("unexpected device {device}");
    }
    let id = device["id"].as_str().context("the device has no ID")?;

    run_on(dail, root, profile, &["serve", "revoke", id, "--yes"])?;
    let listed = run_on(dail, root, profile, &["serve", "devices"])?;
    let left = listed
        .first()
        .and_then(|l| l["data"]["devices"].as_array())
        .map(Vec::len);
    if left != Some(0) {
        bail!("the revoked device is still listed: {listed:?}");
    }
    Ok(())
}
