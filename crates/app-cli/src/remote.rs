//! `dail remote add|status` (ADR 0017): pair this profile with a dail server as a device
//! (`connect_server`, then `trust_server`), and show the link.
//!
//! Each step's result is printed as a JSON line, then `{"ok","name":"remote","link"}`. A pairing
//! that does not complete leaves no link behind (`disconnect_server`).

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::process::ExitCode;

use anyhow::{Context as _, bail};
use app_core::{Command, CoreError, Origin, Secret};
use serde_json::json;

use crate::cli::{GlobalArgs, RemoteCmd};
use crate::output::{emit, result_json};
use crate::runtime::{Front, Listen, connect};
use crate::serve::{DEFAULT_PORT, confirm};

pub async fn run(global: &GlobalArgs, command: &RemoteCmd) -> anyhow::Result<ExitCode> {
    match command {
        RemoteCmd::Add {
            server,
            code,
            fingerprint,
        } => add(global, server, code.as_deref(), fingerprint.as_deref()).await,
        RemoteCmd::Status => status(global).await,
    }
}

/// `host[:port]` → host and port (default [`DEFAULT_PORT`]). An IPv6 address keeps its
/// brackets (`[::1]:7443`), as `connect_server` takes it; a bare one has no port.
pub fn parse_server(text: &str) -> anyhow::Result<(String, u16)> {
    let text = text.trim();
    let (host, port) = if let Some(rest) = text.strip_prefix('[') {
        match rest.split_once(']') {
            Some((_, "")) => (text, None),
            Some((inner, port)) => match port.strip_prefix(':') {
                Some(port) => (&text[..inner.len() + 2], Some(port)),
                None => bail!("expected host[:port], got `{text}`"),
            },
            None => bail!("expected host[:port], got `{text}`"),
        }
    } else {
        match text.matches(':').count() {
            0 => (text, None),
            1 => text
                .split_once(':')
                .map(|(host, port)| (host, Some(port)))
                .unwrap_or((text, None)),
            // A bare IPv6 address.
            _ => (text, None),
        }
    };
    if host.is_empty() {
        bail!("expected host[:port], got `{text}`");
    }
    let port = match port {
        Some(port) => port
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .with_context(|| format!("bad port `{port}`"))?,
        None => DEFAULT_PORT,
    };
    Ok((host.to_owned(), port))
}

/// A fingerprint as the user typed it, as 64 lower-case hex digits: colons and spaces are
/// dropped, nothing else is accepted.
pub fn normalize_fingerprint(text: &str) -> anyhow::Result<String> {
    let hex: String = text
        .chars()
        .filter(|c| *c != ':' && !c.is_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("a fingerprint is 64 hex digits (SHA-256), as `dail serve` prints it");
    }
    Ok(hex)
}

/// `aa:bb:…` in lines of 16 bytes, easier to compare by eye.
fn grouped(fingerprint: &str) -> String {
    let pairs: Vec<&str> = fingerprint
        .as_bytes()
        .chunks(2)
        .filter_map(|pair| std::str::from_utf8(pair).ok())
        .collect();
    pairs
        .chunks(16)
        .map(|line| line.join(":"))
        .collect::<Vec<_>>()
        .join("\n  ")
}

fn read_code() -> anyhow::Result<String> {
    let mut err = std::io::stderr().lock();
    write!(err, "Pairing code (from `dail serve pair` on the server): ")?;
    err.flush()?;
    drop(err);
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .context("cannot read the pairing code")?;
    Ok(line.trim().to_owned())
}

async fn add(
    global: &GlobalArgs,
    server: &str,
    code: Option<&str>,
    fingerprint: Option<&str>,
) -> anyhow::Result<ExitCode> {
    let (host, port) = parse_server(server)?;
    let expected = fingerprint.map(normalize_fingerprint).transpose()?;
    let interactive = std::io::stdin().is_terminal();
    if !interactive && (expected.is_none() || code.is_none()) {
        bail!(
            "stdin is not a terminal: pass --fingerprint (the one `dail serve` printed) and --code (from `dail serve pair`)"
        );
    }
    let front = connect(global, Listen::No).await?;
    let paired = pair(global, &front, &host, port, expected.as_deref(), code).await;
    let ok = matches!(paired, Ok(true));
    if !ok {
        abandon(&front).await;
    }
    let link = front.conn.state().await.map(|state| state.link.clone());
    front.close().await?;
    paired?;
    emit(
        global.pretty,
        &json!({ "ok": ok, "name": "remote", "link": link? }),
    )?;
    if ok {
        let kept = match global.backend {
            crate::cli::BackendKind::Live => "in the OS keyring",
            crate::cli::BackendKind::Memory => "in memory until this process ends",
        };
        let _ = writeln!(
            std::io::stderr().lock(),
            "Paired with {host}:{port}: its certificate is pinned and the device token is kept \
             {kept}. This profile is now linked to the server: it runs no mail sync, rules or \
             sends of its own. This dail version records the link but does not connect to the \
             server yet (remote profiles: ROADMAP SRV-5a); `dail remote status` shows it."
        );
    }
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Removes a link this run left unpinned, so the profile is a plain one again.
async fn abandon(front: &Front) {
    let unpinned = front.conn.state().await.ok().is_some_and(|state| {
        state
            .link
            .as_ref()
            .is_some_and(|link| !link.server.as_ref().is_some_and(|s| s.is_pinned()))
    });
    if unpinned {
        let _ = front
            .conn
            .submit(Command::DisconnectServer { confirm: true }, Origin::Cli)
            .await;
        let _ = front.conn.flush().await;
    }
}

/// `connect_server`, the fingerprint check, `trust_server`: whether the server accepted the
/// code and its certificate is pinned.
async fn pair(
    global: &GlobalArgs,
    front: &Front,
    host: &str,
    port: u16,
    expected: Option<&str>,
    code: Option<&str>,
) -> anyhow::Result<bool> {
    let conn = &front.conn;
    let connected = conn
        .submit(
            Command::ConnectServer {
                host: host.to_owned(),
                port,
            },
            Origin::Cli,
        )
        .await?;
    emit(
        global.pretty,
        &result_json(Some("connect_server"), None, &connected),
    )?;
    if let Err(err) = &connected {
        if matches!(err, CoreError::ProfileHasAccounts { .. }) {
            let _ = writeln!(
                std::io::stderr().lock(),
                "This profile has accounts, so it can't be linked to a dail server. Use another \
                 profile: `dail --profile <name> remote add …`."
            );
        }
        return Ok(false);
    }
    conn.flush().await?;
    let state = conn.state().await?;
    let link = state.link.as_ref();
    let Some(offered) = link
        .and_then(|link| link.server.as_ref())
        .and_then(|server| server.offered.clone())
    else {
        let error = link.and_then(|link| link.error.clone()).unwrap_or_default();
        let _ = writeln!(
            std::io::stderr().lock(),
            "The server at {host}:{port} did not answer: {error}"
        );
        return Ok(false);
    };
    match expected {
        Some(expected) if expected != offered => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "The server at {host}:{port} presents the fingerprint\n  {}\nnot the one given \
                 with --fingerprint. Do not pair: another machine may be answering in its place.",
                grouped(&offered)
            );
            return Ok(false);
        }
        Some(_) => {}
        None => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "The server at {host}:{port} presents the certificate SHA-256 fingerprint\n  {}\n\
                 Compare it with the one `dail serve` printed on the server.",
                grouped(&offered)
            );
            if !confirm("Do they match?")? {
                return Ok(false);
            }
        }
    }
    let code = match code {
        Some(code) => code.to_owned(),
        None => read_code()?,
    };
    let trusted = conn
        .submit(
            Command::TrustServer {
                fingerprint: offered,
                code: Secret::new(code),
            },
            Origin::Cli,
        )
        .await?;
    emit(
        global.pretty,
        &result_json(Some("trust_server"), None, &trusted),
    )?;
    if trusted.is_err() {
        return Ok(false);
    }
    conn.flush().await?;
    let state = conn.state().await?;
    let link = state.link.as_ref();
    if link
        .and_then(|link| link.server.as_ref())
        .is_some_and(app_core::link::ServerEndpoint::is_pinned)
    {
        return Ok(true);
    }
    let error = link.and_then(|link| link.error.clone()).unwrap_or_default();
    let _ = writeln!(
        std::io::stderr().lock(),
        "Pairing with {host}:{port} failed: {error}"
    );
    Ok(false)
}

async fn status(global: &GlobalArgs) -> anyhow::Result<ExitCode> {
    let front = connect(global, Listen::No).await?;
    let state = front.conn.state().await;
    front.close().await?;
    let state = state?;
    let value = match &state.link {
        None => json!({ "ok": true, "name": "remote_status", "linked": false }),
        Some(link) => {
            let server = link.server.as_ref();
            json!({
                "ok": true,
                "name": "remote_status",
                "linked": true,
                "status": link.status,
                "server": server.map(|s| json!({
                    "host": s.host,
                    "port": s.port,
                    "fingerprint": s.fingerprint,
                    "offered": s.offered,
                    "paired": s.is_pinned(),
                })),
                "revision": link.revision,
                "queued": link.queued.len(),
                "rejected": link.rejected,
                "error": link.error,
            })
        }
    };
    emit(global.pretty, &value)?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_parse_with_and_without_ports() {
        let parse = |text| parse_server(text).unwrap();
        assert_eq!(parse("home.example.org"), ("home.example.org".into(), 7443));
        assert_eq!(parse(" 100.64.0.7:8443 "), ("100.64.0.7".into(), 8443));
        assert_eq!(parse("[::1]:9000"), ("[::1]".into(), 9000));
        assert_eq!(parse("[fd7a::1]"), ("[fd7a::1]".into(), 7443));
        assert_eq!(parse("fd7a::1"), ("fd7a::1".into(), 7443));
        for bad in [
            "",
            ":7443",
            "host:0",
            "host:http",
            "[::1",
            "[::1]x",
            "host:99999",
        ] {
            assert!(parse_server(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn fingerprints_are_compared_as_hex() {
        let hex = "ab".repeat(32);
        let colons = vec!["AB"; 32].join(":");
        assert_eq!(normalize_fingerprint(&colons).unwrap(), hex);
        assert_eq!(normalize_fingerprint(&format!(" {hex}\n")).unwrap(), hex);
        assert!(normalize_fingerprint(&hex[..62]).is_err());
        assert!(normalize_fingerprint(&"zz".repeat(32)).is_err());
        assert_eq!(
            grouped(&hex),
            format!(
                "{}\n  {}",
                vec!["ab"; 16].join(":"),
                vec!["ab"; 16].join(":")
            )
        );
    }
}
