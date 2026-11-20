//! `dail serve` (ADR 0017, `ARCHITECTURE.md §5.6`): the dail server, a headless owner of the
//! profile that serves paired devices over TLS; `dail serve pair|devices|revoke`: aliases for
//! `start_pairing`, `list_devices` and `revoke_device` on it.
//!
//! The running server writes `<profile>/serve/listening.json` (its bound addresses, the
//! advertised hosts and the certificate fingerprint) for `dail serve pair`, and removes it when
//! it stops. Every owner of the profile removes it when it takes the lock
//! (`runtime::open_owned`), so while the profile is owned the file exists only if the owner is
//! `dail serve`, even after a crash left one behind.

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::{Context as _, bail};
use app_core::{Command, DeviceId, Origin};
use app_io::ProfileDirs;
use app_sync::serve::{self, DailServer, ServeOptions, ServerIdentity};
use app_sync::{Connection, IpcClient, IpcEndpoint, RandomSeeds};
use serde_json::{Value, json};

use crate::cli::{GlobalArgs, ServeArgs, ServeCmd};
use crate::output::{emit, result_json};
use crate::runtime::{self, Listen, Serve};

/// The port `dail serve` listens on without `--listen`, and `dail remote add` dials without
/// one: unprivileged, and not the port of a common service.
pub const DEFAULT_PORT: u16 = 7443;

/// Where `dail serve` listens without `--listen`: loopback only, so nothing is reachable from
/// the network until the user names an address.
pub fn default_listen() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT))
}

/// The file a running `dail serve` describes itself in, under `<profile>/serve/`.
const LISTENING_FILE: &str = "listening.json";

/// `serve/listening.json` of the profile at `root`.
pub fn listening_file(root: &Path) -> PathBuf {
    ProfileDirs::at(root).serve_dir().join(LISTENING_FILE)
}

pub async fn run(
    global: &GlobalArgs,
    args: &ServeArgs,
    command: Option<&ServeCmd>,
) -> anyhow::Result<ExitCode> {
    match command {
        None => serve(global, args).await,
        Some(ServeCmd::Pair { role, name }) => pair(global, (*role).into(), name).await,
        Some(ServeCmd::Devices) => devices(global).await,
        Some(ServeCmd::Revoke { id, yes }) => revoke(global, id, *yes).await,
    }
}

/// The profile directory `dail serve` works on: the live profile, or `--profile-dir` with
/// `--backend memory` (whose sessions are private otherwise).
fn serve_root(global: &GlobalArgs) -> anyhow::Result<PathBuf> {
    runtime::profile_root(global)?.context(
        "`dail serve --backend memory` needs --profile-dir <dir>: devices pair through the \
         profile's socket and are kept in its config.toml",
    )
}

/// Ctrl+C, or SIGTERM on Unix (`systemctl stop`).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

async fn serve(global: &GlobalArgs, args: &ServeArgs) -> anyhow::Result<ExitCode> {
    let root = serve_root(global)?;
    let owned = runtime::own(global, Serve::Yes).await?;
    let started = start(&root, &owned, args).await;
    let result = match started {
        Ok(server) => {
            shutdown_signal().await;
            let _ = writeln!(
                std::io::stderr().lock(),
                "dail serve: stopping (saving drafts, waiting for running work)"
            );
            drop(server);
            Ok(ExitCode::SUCCESS)
        }
        Err(err) => Err(err),
    };
    let _ = std::fs::remove_file(listening_file(&root));
    let closed = owned.close().await;
    match (result, closed) {
        (result, Ok(())) => result,
        // The start error is the one to report; the failed shutdown is logged.
        (Err(err), Err(closing)) => {
            tracing::warn!(error = %closing, "the session did not shut down cleanly");
            Err(err)
        }
        (Ok(_), Err(closing)) => Err(closing),
    }
}

/// Sweeps old uploads, loads the identity, listens and reports: the running server.
async fn start(
    root: &Path,
    owned: &runtime::Owned,
    args: &ServeArgs,
) -> anyhow::Result<DailServer> {
    let hub = owned
        .hub
        .clone()
        .context("the session was opened without the dail server's devices")?;
    let serve_dir = ProfileDirs::at(root).serve_dir();
    let uploads = serve_dir.join("uploads");
    let removed = serve::sweep_uploads(
        &uploads,
        &owned.session.state(),
        serve::UPLOAD_MAX_AGE,
        SystemTime::now(),
    );
    if removed > 0 {
        tracing::info!(removed, "removed old uploads no draft names");
    }
    let identity = ServerIdentity::load_or_create(&serve_dir, &args.advertise)?;
    let mut options = ServeOptions::new(uploads);
    options.allow_send = args.allow_send;
    let mut server = DailServer::start(
        &identity,
        hub,
        owned.session.client(),
        Arc::new(RandomSeeds),
        options,
    )?;
    let wanted = if args.listen.is_empty() {
        vec![default_listen()]
    } else {
        args.listen.clone()
    };
    let mut bound = Vec::with_capacity(wanted.len());
    for addr in wanted {
        bound.push(server.listen(addr).await?);
    }
    let listening = json!({
        "pid": std::process::id(),
        "addresses": bound.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "advertise": args.advertise,
        "fingerprint": identity.fingerprint(),
        "allow_send": args.allow_send,
    });
    write_private(&listening_file(root), &listening)?;

    let mut err = std::io::stderr().lock();
    writeln!(err, "dail serve: profile {}", root.display())?;
    for addr in &bound {
        writeln!(err, "dail serve: listening on {addr} (TLS)")?;
    }
    writeln!(
        err,
        "dail serve: certificate SHA-256 fingerprint {}",
        identity.fingerprint()
    )?;
    if args.allow_send {
        writeln!(
            err,
            "dail serve: --allow-send: agent devices may send without approval"
        )?;
    }
    writeln!(
        err,
        "dail serve: pair a device with `dail serve pair --role user --name <device>`; Ctrl+C or SIGTERM stops"
    )?;
    Ok(server)
}

/// Writes `value` as JSON to `path`, readable by the user only (Unix).
fn write_private(path: &Path, value: &Value) -> anyhow::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("cannot write {}", path.display()))?;
    serde_json::to_writer(&mut file, value)?;
    writeln!(file)?;
    Ok(())
}

/// A connection to the running `dail serve` of the profile, and what it wrote about itself.
async fn attach_server(global: &GlobalArgs) -> anyhow::Result<(Connection, Value)> {
    let root = serve_root(global)?;
    let not_running = || {
        format!(
            "dail serve is not running for the profile {}; start it with `dail serve`",
            root.display()
        )
    };
    let client = IpcClient::connect(&IpcEndpoint::for_profile(&root))
        .await
        .map_err(|_| anyhow::anyhow!(not_running()))?;
    let listening = std::fs::read_to_string(listening_file(&root))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .with_context(|| {
            format!(
                "the dail process that owns the profile {} is not `dail serve`; close it and run `dail serve`",
                root.display()
            )
        })?;
    Ok((Connection::Attached(client), listening))
}

/// `host:port` of each advertised host (with the first listener's port), then each bound
/// address: where a device can reach the server.
fn reachable_at(listening: &Value) -> Vec<String> {
    let strings = |key: &str| -> Vec<String> {
        listening[key]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let bound = strings("addresses");
    let port = bound
        .first()
        .and_then(|addr| addr.parse::<SocketAddr>().ok())
        .map_or(DEFAULT_PORT, |addr| addr.port());
    let mut at: Vec<String> = strings("advertise")
        .iter()
        .map(|host| {
            if host.contains(':') && !host.starts_with('[') {
                format!("[{host}]:{port}")
            } else {
                format!("{host}:{port}")
            }
        })
        .collect();
    at.extend(bound);
    at
}

async fn pair(
    global: &GlobalArgs,
    role: app_core::DeviceRole,
    name: &str,
) -> anyhow::Result<ExitCode> {
    let (conn, listening) = attach_server(global).await?;
    let before = conn.state().await?.status.serve_error.clone();
    let result = conn
        .submit(
            Command::StartPairing {
                role,
                name: name.to_owned(),
            },
            Origin::Cli,
        )
        .await?;
    let receipt = match result {
        Ok(receipt) => receipt,
        Err(err) => {
            emit(
                global.pretty,
                &result_json(Some("start_pairing"), None, &Err(err)),
            )?;
            return Ok(ExitCode::FAILURE);
        }
    };
    conn.flush().await?;
    let after = conn.state().await?.status.serve_error.clone();
    if let Some(error) = after.filter(|error| Some(error) != before.as_ref()) {
        emit(
            global.pretty,
            &json!({ "ok": false, "name": "pairing", "error": error }),
        )?;
        return Ok(ExitCode::FAILURE);
    }
    let data = &receipt.data;
    let code = data["code"].as_str().unwrap_or_default();
    let expires_at_ms = data["expires_at_ms"].as_i64().unwrap_or_default();
    let fingerprint = listening["fingerprint"].as_str().unwrap_or_default();
    let at = reachable_at(&listening);
    emit(
        global.pretty,
        &json!({
            "ok": true,
            "name": "pairing",
            "code": code,
            "role": data["role"],
            "device_name": data["name"],
            "expires_at_ms": expires_at_ms,
            "fingerprint": fingerprint,
            "addresses": at,
        }),
    )?;
    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    let minutes = (expires_at_ms - now_ms).max(0) / 60_000;
    let mut err = std::io::stderr().lock();
    writeln!(
        err,
        "Pairing code {code} for {} device \"{}\": single use, valid for {minutes} more minutes.",
        role.as_str(),
        data["name"].as_str().unwrap_or_default()
    )?;
    writeln!(err, "Server certificate SHA-256 fingerprint: {fingerprint}")?;
    for addr in &at {
        writeln!(err, "On the device: dail remote add {addr}")?;
    }
    writeln!(
        err,
        "The device must show the same fingerprint before you enter the code."
    )?;
    Ok(ExitCode::SUCCESS)
}

async fn devices(global: &GlobalArgs) -> anyhow::Result<ExitCode> {
    let front = runtime::connect_serving(global, Listen::No, Serve::Yes).await?;
    let result = front
        .conn
        .submit(Command::ListDevices {}, Origin::Cli)
        .await;
    front.close().await?;
    let result = result?;
    emit(
        global.pretty,
        &result_json(Some("list_devices"), None, &result),
    )?;
    Ok(if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Asks `question` on stderr and reads a yes from stdin; anything else is a no.
pub fn confirm(question: &str) -> anyhow::Result<bool> {
    let mut err = std::io::stderr().lock();
    write!(err, "{question} [y/N] ")?;
    err.flush()?;
    drop(err);
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .context("cannot read the answer")?;
    Ok(matches!(line.trim(), "y" | "Y" | "yes" | "Yes" | "YES"))
}

async fn revoke(global: &GlobalArgs, id: &str, yes: bool) -> anyhow::Result<ExitCode> {
    let id: DeviceId = serde_json::from_value(json!(id.trim()))
        .with_context(|| format!("`{id}` is not a device ID (see `dail serve devices`)"))?;
    if !yes && !std::io::stdin().is_terminal() {
        bail!("stdin is not a terminal: pass --yes to revoke the device");
    }
    let front = runtime::connect_serving(global, Listen::No, Serve::Yes).await?;
    let outcome = revoke_on(global, &front.conn, id, yes).await;
    front.close().await?;
    outcome
}

async fn revoke_on(
    global: &GlobalArgs,
    conn: &Connection,
    id: DeviceId,
    yes: bool,
) -> anyhow::Result<ExitCode> {
    let state = conn.state().await?;
    let before = state.status.serve_error.clone();
    if !yes {
        let name = state
            .devices
            .get(&id)
            .map_or_else(|| "?".to_owned(), |device| device.name.clone());
        if !confirm(&format!(
            "Revoke the device \"{name}\" ({id})? Its token stops working and it is disconnected at once."
        ))? {
            let _ = writeln!(std::io::stderr().lock(), "Not revoked.");
            return Ok(ExitCode::FAILURE);
        }
    }
    let result = conn
        .submit(Command::RevokeDevice { id, confirm: true }, Origin::Cli)
        .await?;
    emit(
        global.pretty,
        &result_json(Some("revoke_device"), None, &result),
    )?;
    if result.is_err() {
        return Ok(ExitCode::FAILURE);
    }
    conn.flush().await?;
    let after = conn.state().await?.status.serve_error.clone();
    if let Some(error) = after.filter(|error| Some(error) != before.as_ref()) {
        emit(
            global.pretty,
            &json!({ "ok": false, "name": "revoke_device", "error": error }),
        )?;
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_reach_the_advertised_hosts_then_the_bound_addresses() {
        let listening = json!({
            "addresses": ["100.101.102.103:7443", "[fd7a::1]:7443"],
            "advertise": ["home.tail1234.ts.net", "fd7a::1"],
        });
        assert_eq!(
            reachable_at(&listening),
            [
                "home.tail1234.ts.net:7443",
                "[fd7a::1]:7443",
                "100.101.102.103:7443",
                "[fd7a::1]:7443",
            ]
        );
        assert!(reachable_at(&json!({})).is_empty());
    }

    #[test]
    fn the_default_listen_address_is_loopback() {
        assert!(default_listen().ip().is_loopback());
        assert_eq!(default_listen().port(), DEFAULT_PORT);
    }
}
