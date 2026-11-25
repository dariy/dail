//! The desktop beside a running `dail serve` (ADR 0017, SRV-10): while `dail serve` owns the
//! profile, the GUI (which must own the profile it shows) opens its companion profile instead,
//! `local` (`<name>-local` for a profile other than `default`; with `--backend memory
//! --profile-dir <dir>`, `<dir>-local`). The companion is a device of that server: on its first
//! start it is paired over loopback with a code the server opens through the profile socket
//! (`start_pairing` as `cli`), and it pins the fingerprint that `dail serve` wrote in its
//! `serve/listening.json` (readable by the user only, like the socket), so no code or
//! fingerprint is typed. Later starts open it as the remote profile it is.

use std::io::Write as _;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use app_core::{Command, DeviceRole, Origin, Secret};
use app_sync::{IpcClient, IpcEndpoint, Session, SyncError};
use serde_json::Value;

use crate::cli::{BackendKind, GlobalArgs};
use crate::runtime::{self, Owned, Serve};

/// The name of the companion of the profile `profile`.
pub fn companion_name(profile: &str) -> String {
    if profile == "default" {
        "local".to_owned()
    } else {
        format!("{profile}-local")
    }
}

/// The companion of `--profile-dir <dir>`: `<dir>-local`, beside it.
fn companion_dir(dir: &Path) -> PathBuf {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push("-local");
    dir.with_file_name(name)
}

/// The options of the companion profile of `global`'s.
fn companion(global: &GlobalArgs) -> GlobalArgs {
    let mut local = global.clone();
    match &global.profile_dir {
        Some(dir) => local.profile_dir = Some(companion_dir(dir)),
        None => local.profile = companion_name(&global.profile),
    }
    local
}

/// The selected profile's `dail serve`, if one owns it: what it wrote about itself, and a
/// connection to its socket.
async fn running_server(global: &GlobalArgs) -> anyhow::Result<Option<(Value, IpcClient)>> {
    if global.backend == BackendKind::Memory && global.profile_dir.is_none() {
        return Ok(None);
    }
    let Some(root) = runtime::profile_root(global)? else {
        return Ok(None);
    };
    let Some(listening) = std::fs::read_to_string(crate::serve::listening_file(&root))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    else {
        return Ok(None);
    };
    match IpcClient::connect(&IpcEndpoint::for_profile(&root)).await {
        Ok(client) => Ok(Some((listening, client))),
        // A `dail serve` of another build: say so rather than open the companion.
        Err(err @ SyncError::OwnerMismatch { .. }) => Err(err.into()),
        // Nobody listens: the file is stale, the profile is free.
        Err(_) => Ok(None),
    }
}

/// Where this machine dials its own server: a loopback address of a listener (an unspecified
/// one, `0.0.0.0` or `::`, is reached on loopback too), else the first listener's address.
/// `host` as `connect_server` takes it (IPv6 in brackets).
pub fn loopback_endpoint(listening: &Value) -> Option<(String, u16)> {
    let bound: Vec<SocketAddr> = listening["addresses"]
        .as_array()?
        .iter()
        .filter_map(|a| a.as_str()?.parse().ok())
        .collect();
    let on_loopback = |addr: &SocketAddr| -> Option<SocketAddr> {
        match addr.ip() {
            ip if ip.is_loopback() => Some(*addr),
            IpAddr::V4(ip) if ip.is_unspecified() => {
                Some(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), addr.port()))
            }
            IpAddr::V6(ip) if ip.is_unspecified() => {
                Some(SocketAddr::new(Ipv6Addr::LOCALHOST.into(), addr.port()))
            }
            _ => None,
        }
    };
    let addr = bound
        .iter()
        .find_map(on_loopback)
        .or_else(|| bound.first().copied())?;
    let host = match addr.ip() {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    Some((host, addr.port()))
}

/// Opens the profile the GUI shows: the selected one, or, while `dail serve` owns it, its
/// companion, paired with that server first if it is not yet.
pub async fn own_for_gui(global: &GlobalArgs) -> anyhow::Result<Owned> {
    let Some((listening, server)) = running_server(global).await? else {
        return runtime::own(global, Serve::No).await;
    };
    let local = companion(global);
    let shown = local
        .profile_dir
        .as_ref()
        .map_or_else(|| local.profile.clone(), |dir| dir.display().to_string());
    let _ = writeln!(
        std::io::stderr().lock(),
        "dail: `dail serve` owns this profile; opening the profile {shown}, a device of it"
    );
    let owned = runtime::own(&local, Serve::No).await?;
    if let Err(err) = pair_if_needed(&owned.session, &listening, &server).await {
        let _ = owned.close().await;
        return Err(err.context(format!(
            "cannot pair the profile {shown} with the dail serve running on this machine"
        )));
    }
    Ok(owned)
}

/// Pairs `session` (the companion) with the local server, unless it already is a device of a
/// server.
async fn pair_if_needed(
    session: &Session,
    listening: &Value,
    server: &IpcClient,
) -> anyhow::Result<()> {
    let state = session.state();
    if state
        .link
        .as_ref()
        .and_then(|link| link.server.as_ref())
        .is_some_and(app_core::ServerEndpoint::is_pinned)
    {
        return Ok(());
    }
    let (host, port) = loopback_endpoint(listening)
        .context("`dail serve` wrote no address it listens on (serve/listening.json)")?;
    let fingerprint = listening["fingerprint"]
        .as_str()
        .context("`dail serve` wrote no certificate fingerprint (serve/listening.json)")?
        .to_owned();
    tracing::info!(%host, port, "pairing the companion profile with the local dail serve");
    let connected = session
        .submit(
            Command::ConnectServer {
                host: host.clone(),
                port,
            },
            Origin::Cli,
        )
        .await?;
    if let Err(err) = connected {
        bail!("{err}");
    }
    // Only this session's effects (the probe): the server's are not waited for.
    session.flush().await?;
    let offered = session
        .state()
        .link
        .as_ref()
        .and_then(|link| link.server.as_ref().and_then(|s| s.offered.clone()));
    let outcome = trust(session, server, offered, &fingerprint).await;
    if outcome.is_err() {
        // Leave a plain profile behind, not a half-made link.
        let _ = session
            .submit(Command::DisconnectServer { confirm: true }, Origin::Cli)
            .await;
        let _ = session.flush().await;
    }
    outcome
}

/// Checks the offered certificate against `dail serve`'s own, then opens a code on the server
/// and redeems it.
async fn trust(
    session: &Session,
    server: &IpcClient,
    offered: Option<String>,
    fingerprint: &str,
) -> anyhow::Result<()> {
    let link_error = || {
        session
            .state()
            .link
            .as_ref()
            .and_then(|link| link.error.clone())
            .unwrap_or_default()
    };
    let Some(offered) = offered else {
        bail!("the server did not answer on loopback: {}", link_error());
    };
    if offered != fingerprint {
        bail!(
            "the server on loopback presents another certificate than the one `dail serve` wrote; not pairing"
        );
    }
    let opened = server
        .submit(
            &Command::StartPairing {
                role: DeviceRole::User,
                name: "local".to_owned(),
            },
            Origin::Cli,
        )
        .await?
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    let code = opened.data["code"]
        .as_str()
        .context("the server's answer has no pairing code")?
        .to_owned();
    let trusted = session
        .submit(
            Command::TrustServer {
                fingerprint: offered,
                code: Secret::new(code),
            },
            Origin::Cli,
        )
        .await?;
    if let Err(err) = trusted {
        bail!("{err}");
    }
    session.flush().await?;
    let pinned = session
        .state()
        .link
        .as_ref()
        .and_then(|link| link.server.as_ref())
        .is_some_and(app_core::ServerEndpoint::is_pinned);
    if !pinned {
        bail!("pairing failed: {}", link_error());
    }
    tracing::info!("the companion profile is a device of the local dail serve");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_companion_of_default_is_local() {
        assert_eq!(companion_name("default"), "local");
        assert_eq!(companion_name("work"), "work-local");
        assert_eq!(
            companion_dir(Path::new("/tmp/p")),
            PathBuf::from("/tmp/p-local")
        );
    }

    #[test]
    fn the_server_is_dialed_on_loopback() {
        let at = |addresses: Value| loopback_endpoint(&json!({ "addresses": addresses }));
        assert_eq!(
            at(json!(["127.0.0.1:7443"])),
            Some(("127.0.0.1".into(), 7443))
        );
        assert_eq!(
            at(json!(["100.64.0.7:7443", "0.0.0.0:8443"])),
            Some(("127.0.0.1".into(), 8443))
        );
        assert_eq!(at(json!(["[::]:7443"])), Some(("[::1]".into(), 7443)));
        assert_eq!(
            at(json!(["100.64.0.7:7443"])),
            Some(("100.64.0.7".into(), 7443))
        );
        assert_eq!(at(json!([])), None);
        assert_eq!(loopback_endpoint(&json!({})), None);
    }
}
