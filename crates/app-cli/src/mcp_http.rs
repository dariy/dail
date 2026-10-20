//! `dail mcp --http <addr>` (ADR 0007): the MCP server over Streamable HTTP on loopback, with a
//! bearer token kept in the profile.

use std::io::Write as _;
use std::net::SocketAddr;
use std::path::Path;
use std::process::ExitCode;

use anyhow::Context as _;
use app_io::ProfileDirs;

use crate::cli::GlobalArgs;
use crate::runtime;

/// The profile's token: read from `mcp-token`, or made (32 random bytes, hex) and written with
/// user-only permissions.
pub fn token(file: &Path) -> anyhow::Result<String> {
    if let Ok(text) = std::fs::read_to_string(file) {
        let token = text.trim().to_owned();
        if token.len() >= 32 {
            return Ok(token);
        }
    }
    let token = fresh_token()?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut out = options
        .open(file)
        .with_context(|| format!("cannot write {}", file.display()))?;
    writeln!(out, "{token}")?;
    Ok(token)
}

fn fresh_token() -> anyhow::Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| anyhow::anyhow!("no random source: {err}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub async fn serve(
    global: &GlobalArgs,
    allow_send: bool,
    addr: SocketAddr,
) -> anyhow::Result<ExitCode> {
    let root = runtime::profile_root(global)?;
    let (token, place) = match &root {
        Some(root) => {
            let file = ProfileDirs::at(root).root().join("mcp-token");
            (token(&file)?, format!("the token is in {}", file.display()))
        }
        // A private memory session: the token only lives as long as this process.
        None => {
            let token = fresh_token()?;
            let place = format!("token {token}");
            (token, place)
        }
    };
    let front = runtime::connect(global, runtime::Listen::Yes).await?;
    let mut err = std::io::stderr().lock();
    writeln!(
        err,
        "dail: MCP over HTTP at http://{addr}{} ({place}); Ctrl+C stops",
        app_mcp::http::PATH
    )?;
    drop(err);
    app_mcp::http::serve_http(front.conn.clone(), allow_send, addr, token, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    front.close().await?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_is_made_once_and_kept() {
        let dir = std::env::temp_dir().join(format!("dail-mcp-token-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("mcp-token");
        let first = token(&file).unwrap();
        assert_eq!(first.len(), 64);
        assert_eq!(token(&file).unwrap(), first);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
