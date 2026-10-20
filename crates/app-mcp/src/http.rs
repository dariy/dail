//! `dail mcp --http` (ADR 0007): the MCP server over Streamable HTTP on a loopback address,
//! for clients that can't start `dail mcp` themselves.
//!
//! Only loopback addresses are served. Every request must carry `Authorization: Bearer
//! <token>` (compared in constant time); the token lives in the profile (`mcp-token`, 0600),
//! so only the user's own processes can read it. rmcp checks `Host` against loopback names
//! (DNS rebinding), and a browser page can't send the header without a CORS preflight, which
//! is never answered.

use std::convert::Infallible;
use std::net::SocketAddr;

use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;

use crate::server::{DailServer, McpError};
use app_sync::Connection;

/// The path clients connect to: `http://127.0.0.1:<port>/mcp`.
pub const PATH: &str = "/mcp";

/// Whether `header` is `Bearer <token>`, compared without an early exit.
pub fn authorized(header: Option<&str>, token: &str) -> bool {
    let Some(given) = header.and_then(|h| h.strip_prefix("Bearer ")) else {
        return false;
    };
    let (a, b) = (given.trim().as_bytes(), token.as_bytes());
    let mut diff = a.len() ^ b.len();
    for (i, byte) in b.iter().enumerate() {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ byte);
    }
    diff == 0 && !token.is_empty()
}

/// Binds `addr` (a loopback address) and serves MCP until `stop` resolves.
pub async fn serve_http(
    conn: Connection,
    allow_send: bool,
    addr: SocketAddr,
    token: String,
    stop: impl std::future::Future<Output = ()>,
) -> Result<(), McpError> {
    if !addr.ip().is_loopback() {
        return Err(McpError::Serve(format!(
            "{addr} is not a loopback address; dail serves MCP over HTTP only on this machine"
        )));
    }
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|err| McpError::Serve(format!("{addr}: {err}")))?;
    let service = StreamableHttpService::new(
        move || Ok(DailServer::new(conn.clone(), allow_send)),
        std::sync::Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    tracing::info!(%addr, "MCP over HTTP");
    tokio::pin!(stop);
    loop {
        let accepted = tokio::select! {
            () = &mut stop => return Ok(()),
            accepted = listener.accept() => accepted,
        };
        let (stream, peer) = match accepted {
            Ok(accepted) => accepted,
            Err(err) => {
                tracing::warn!(error = %err, "MCP over HTTP: accept failed");
                continue;
            }
        };
        let service = service.clone();
        let token = token.clone();
        tokio::spawn(async move {
            let handler = hyper::service::service_fn(move |req: Request<Incoming>| {
                let service = service.clone();
                let token = token.clone();
                async move {
                    let header = req
                        .headers()
                        .get(hyper::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok());
                    if !authorized(header, &token) {
                        return Ok::<_, Infallible>(refuse(StatusCode::UNAUTHORIZED));
                    }
                    if req.uri().path() != PATH {
                        return Ok(refuse(StatusCode::NOT_FOUND));
                    }
                    tower_call(service, req).await
                }
            });
            if let Err(err) = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), handler)
                .await
            {
                tracing::debug!(%peer, error = %err, "MCP over HTTP: connection ended");
            }
        });
    }
}

/// rmcp's service is a tower service; hyper-util adapts it.
async fn tower_call(
    service: StreamableHttpService<DailServer, LocalSessionManager>,
    req: Request<Incoming>,
) -> Result<Response<http_body_util::combinators::BoxBody<Bytes, Infallible>>, Infallible> {
    use hyper::service::Service as _;
    hyper_util::service::TowerToHyperService::new(service)
        .call(req)
        .await
}

fn refuse(status: StatusCode) -> Response<http_body_util::combinators::BoxBody<Bytes, Infallible>> {
    let mut response = Response::new(
        Full::new(Bytes::from(status.canonical_reason().unwrap_or("refused"))).boxed(),
    );
    *response.status_mut() = status;
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exact_bearer_token_passes() {
        assert!(authorized(Some("Bearer s3cret"), "s3cret"));
        for header in [
            None,
            Some("s3cret"),
            Some("Bearer s3cre"),
            Some("Bearer s3cretX"),
            Some("Basic s3cret"),
        ] {
            assert!(!authorized(header, "s3cret"), "{header:?}");
        }
        assert!(
            !authorized(Some("Bearer "), ""),
            "an empty token never passes"
        );
    }
}
