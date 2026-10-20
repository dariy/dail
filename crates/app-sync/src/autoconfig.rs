//! `Effect::DiscoverServers` (ADR 0006): a domain's mail servers from autoconfig documents over
//! HTTPS. A minimal HTTP/1.1 client (GET, `Connection: close`, content length or chunked
//! bodies, HTTPS redirects) over the same rustls + app-tls client as IMAP; each try is limited
//! in time and size. The documents are read by `app_io::autoconfig`.

use std::sync::Arc;
use std::time::Duration;

use app_core::Event;
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// How long one URL may take.
const TIMEOUT: Duration = Duration::from_secs(10);
/// The largest response read.
const MAX_BODY: usize = 64 * 1024;
/// Redirects followed per URL.
const MAX_REDIRECTS: usize = 3;

/// An HTTP response: status and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// `Location`, for redirects.
    pub location: Option<String>,
    /// The body (de-chunked).
    pub body: Vec<u8>,
}

/// `https://host/path` split into host and path; `None` for anything but an HTTPS URL.
pub fn split_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://")?;
    let (host, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_owned()), |(h, p)| (h, format!("/{p}")));
    let valid = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    valid.then(|| (host.to_ascii_lowercase(), path))
}

/// Sends `GET path` for `host` over `stream` and reads the response.
pub async fn get<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    host: &str,
    path: &str,
) -> Result<Response, String> {
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: dail\r\nAccept: application/xml, text/xml\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|err| err.to_string())?;
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|err| err.to_string())?;
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..n]);
        if raw.len() > MAX_BODY + 8192 {
            return Err("the answer is too large".to_owned());
        }
    }
    parse_response(&raw)
}

/// Parses a whole HTTP/1.1 response.
pub fn parse_response(raw: &[u8]) -> Result<Response, String> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "not an HTTP answer".to_owned())?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let mut lines = head.split("\r\n");
    let status: u16 = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| "not an HTTP answer".to_owned())?;
    let mut location = None;
    let mut chunked = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "location" => location = Some(value.trim().to_owned()),
            "transfer-encoding" => chunked = value.to_ascii_lowercase().contains("chunked"),
            _ => {}
        }
    }
    let body = &raw[split + 4..];
    let body = if chunked {
        dechunk(body)?
    } else {
        body.to_vec()
    };
    if body.len() > MAX_BODY {
        return Err("the answer is too large".to_owned());
    }
    Ok(Response {
        status,
        location,
        body,
    })
}

fn dechunk(mut body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let line_end = body
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| "a broken chunked answer".to_owned())?;
        let size_text = String::from_utf8_lossy(&body[..line_end]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| "a broken chunked answer".to_owned())?;
        body = &body[line_end + 2..];
        if size == 0 {
            return Ok(out);
        }
        let chunk = body
            .get(..size)
            .ok_or_else(|| "a broken chunked answer".to_owned())?;
        out.extend_from_slice(chunk);
        if out.len() > MAX_BODY {
            return Err("the answer is too large".to_owned());
        }
        body = body.get(size + 2..).unwrap_or_default();
    }
}

/// Fetches `url` over HTTPS, following HTTPS redirects.
async fn fetch(tls: &Arc<ClientConfig>, url: &str) -> Result<Vec<u8>, String> {
    let mut url = url.to_owned();
    for _ in 0..=MAX_REDIRECTS {
        let (host, path) = split_url(&url).ok_or_else(|| format!("{url} is not an HTTPS URL"))?;
        let name = ServerName::try_from(host.clone()).map_err(|err| err.to_string())?;
        let response = tokio::time::timeout(TIMEOUT, async {
            let tcp = TcpStream::connect((host.as_str(), 443))
                .await
                .map_err(|err| format!("{host}: {err}"))?;
            let mut stream = TlsConnector::from(Arc::clone(tls))
                .connect(name, tcp)
                .await
                .map_err(|err| format!("{host}: TLS: {err}"))?;
            get(&mut stream, &host, &path).await
        })
        .await
        .map_err(|_| format!("{host}: timed out"))??;
        match response.status {
            200 => return Ok(response.body),
            301 | 302 | 303 | 307 | 308 => {
                url = response
                    .location
                    .ok_or_else(|| format!("{host}: a redirect without a target"))?;
            }
            status => return Err(format!("{host}: HTTP {status}")),
        }
    }
    Err("too many redirects".to_owned())
}

/// Looks `domain` up at each autoconfig URL in turn and reports the first usable answer.
pub async fn discover(tls: &Arc<ClientConfig>, email: String, domain: &str) -> Event {
    let mut last = String::from("no autoconfig document");
    for url in app_io::autoconfig::urls(domain) {
        match fetch(tls, &url).await {
            Ok(body) => {
                let xml = String::from_utf8_lossy(&body);
                if let Some(servers) = app_io::autoconfig::parse(&xml, &email) {
                    return Event::ServersDiscovered {
                        email,
                        imap: servers.imap,
                        smtp: servers.smtp,
                        source: url,
                    };
                }
                last = format!("{url}: no usable servers");
            }
            Err(error) => last = error,
        }
    }
    Event::DiscoveryFailed { email, error: last }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_https_only() {
        assert_eq!(
            split_url("https://autoconfig.example.org/mail/config-v1.1.xml"),
            Some((
                "autoconfig.example.org".into(),
                "/mail/config-v1.1.xml".into()
            ))
        );
        assert_eq!(
            split_url("https://Example.org"),
            Some(("example.org".into(), "/".into()))
        );
        assert!(split_url("http://example.org/").is_none());
        assert!(split_url("https://evil host/").is_none());
    }

    #[test]
    fn responses_are_parsed_with_chunks_and_redirects() {
        let plain = parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello").unwrap();
        assert_eq!(
            (plain.status, plain.body.as_slice()),
            (200, b"hello".as_slice())
        );
        let chunked = parse_response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nhel\r\n2;x=y\r\nlo\r\n0\r\n\r\n",
        )
        .unwrap();
        assert_eq!(chunked.body, b"hello");
        let moved =
            parse_response(b"HTTP/1.1 301 Moved\r\nLocation: https://x.org/a\r\n\r\n").unwrap();
        assert_eq!(moved.location.as_deref(), Some("https://x.org/a"));
        assert!(parse_response(b"garbage").is_err());
        assert!(
            parse_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n").is_err()
        );
    }

    #[tokio::test]
    async fn a_get_over_any_stream() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        let serve = tokio::spawn(async move {
            let mut request = vec![0_u8; 1024];
            let n = server.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]).into_owned();
            server
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .await
                .unwrap();
            drop(server);
            request
        });
        let response = get(&mut client, "example.org", "/x").await.unwrap();
        assert_eq!(response.body, b"ok");
        let request = serve.await.unwrap();
        assert!(
            request.starts_with("GET /x HTTP/1.1\r\nHost: example.org\r\n"),
            "{request}"
        );
        assert!(!request.contains('@'), "the address is never sent");
    }
}
