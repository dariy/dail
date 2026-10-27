//! HTTPS GET for updates of dail (Wave 7 S1, ADR 0011): hyper's HTTP/1.1 client over the same
//! rustls + app-tls client as IMAP. Redirects are followed at most [`MAX_REDIRECTS`] times and
//! only to [`ALLOWED_HOSTS`]. The request carries `User-Agent: dail/<version>` and nothing else
//! about the user. The signature of the manifest is the security boundary; TLS and the host list
//! only cut the noise.

use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt, Empty};
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// The hosts update requests may go to: the release page and GitHub's asset storage.
pub const ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

/// The signed manifest of the newest release, and its signature.
pub const MANIFEST_URL: &str =
    "https://github.com/dariy/dail/releases/latest/download/manifest.json";
/// [`MANIFEST_URL`]'s minisign signature.
pub const SIGNATURE_URL: &str =
    "https://github.com/dariy/dail/releases/latest/download/manifest.json.minisig";

/// Redirects followed per request.
pub const MAX_REDIRECTS: usize = 5;

/// How long connecting and the response head may take.
const HEAD_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the body may stall between two chunks.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// `https://host/path` of an allowed host, split; an error for anything else.
pub fn allowed_url(url: &str) -> Result<(String, String), String> {
    let (host, path) =
        crate::autoconfig::split_url(url).ok_or_else(|| format!("not an HTTPS URL: {url}"))?;
    if !ALLOWED_HOSTS.contains(&host.as_str()) {
        return Err(format!("{host} is not a GitHub release host"));
    }
    Ok((host, path))
}

/// The URL a redirect from `host` with `location` leads to: absolute HTTPS, or a path on the
/// same host.
pub fn redirect_target(host: &str, location: &str) -> String {
    if location.starts_with('/') && !location.starts_with("//") {
        format!("https://{host}{location}")
    } else {
        location.to_owned()
    }
}

/// An HTTPS client for update requests.
#[derive(Clone)]
pub struct Https {
    tls: Arc<ClientConfig>,
    user_agent: String,
}

impl Https {
    /// A client that sends `User-Agent: dail/<version>`.
    pub fn new(tls: Arc<ClientConfig>, version: &str) -> Self {
        Self {
            tls,
            user_agent: format!("dail/{version}"),
        }
    }

    /// GETs `url` and follows redirects within [`ALLOWED_HOSTS`]; the response of the last hop,
    /// which is `200 OK`.
    pub async fn open(&self, url: &str) -> Result<Response<Incoming>, String> {
        let mut url = url.to_owned();
        for _ in 0..=MAX_REDIRECTS {
            let (host, path) = allowed_url(&url)?;
            let response = tokio::time::timeout(HEAD_TIMEOUT, self.get_once(&host, &path))
                .await
                .map_err(|_| format!("{host} did not answer in time"))??;
            let status = response.status();
            if status.is_redirection() {
                let location = response
                    .headers()
                    .get(hyper::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| format!("{host} redirected without a location"))?;
                url = redirect_target(&host, location);
                continue;
            }
            if status != hyper::StatusCode::OK {
                return Err(format!("{host} answered {status} for {path}"));
            }
            return Ok(response);
        }
        Err(format!("more than {MAX_REDIRECTS} redirects"))
    }

    /// GETs `url` and reads the whole body, at most `max` bytes.
    pub async fn get_bytes(&self, url: &str, max: usize) -> Result<Vec<u8>, String> {
        let mut body = self.open(url).await?.into_body();
        let mut data = Vec::new();
        while let Some(chunk) = next_chunk(&mut body).await? {
            data.extend_from_slice(&chunk);
            if data.len() > max {
                return Err(format!("the answer is larger than {max} bytes"));
            }
        }
        Ok(data)
    }

    async fn get_once(&self, host: &str, path: &str) -> Result<Response<Incoming>, String> {
        let name = ServerName::try_from(host.to_owned()).map_err(|err| err.to_string())?;
        let tcp = TcpStream::connect((host, 443))
            .await
            .map_err(|err| format!("cannot connect to {host}: {err}"))?;
        let stream = TlsConnector::from(Arc::clone(&self.tls))
            .connect(name, tcp)
            .await
            .map_err(|err| crate::tls::with_hint(format!("TLS with {host} failed: {err}"), &err))?;
        get_on(stream, host, path, &self.user_agent).await
    }
}

/// Sends `GET path` for `host` over `stream` (one request, then the connection closes).
pub async fn get_on<S>(
    stream: S,
    host: &str,
    path: &str,
    user_agent: &str,
) -> Result<Response<Incoming>, String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|err| err.to_string())?;
    tokio::spawn(async move {
        if let Err(err) = connection.await {
            tracing::debug!(error = %err, "update connection closed");
        }
    });
    let request = Request::get(path)
        .header(hyper::header::HOST, host)
        .header(hyper::header::USER_AGENT, user_agent)
        .header(hyper::header::CONNECTION, "close")
        .body(Empty::<Bytes>::new())
        .map_err(|err| err.to_string())?;
    sender
        .send_request(request)
        .await
        .map_err(|err| format!("{host}: {err}"))
}

/// The next data chunk of `body`, or `None` at its end; fails if it stalls for [`IDLE_TIMEOUT`].
pub async fn next_chunk(body: &mut Incoming) -> Result<Option<Bytes>, String> {
    loop {
        let frame = tokio::time::timeout(IDLE_TIMEOUT, body.frame())
            .await
            .map_err(|_| "the download stalled".to_owned())?;
        match frame {
            None => return Ok(None),
            Some(Err(err)) => return Err(err.to_string()),
            Some(Ok(frame)) => {
                if let Ok(data) = frame.into_data() {
                    return Ok(Some(data));
                }
            }
        }
    }
}

/// `Effect::CheckForUpdates`: fetches the manifest and its signature and verifies them with the
/// built-in keys.
pub async fn check(https: &Https, running: &str) -> app_core::Event {
    match check_with(https, running, app_io::update::UPDATE_PUBLIC_KEYS).await {
        Ok(manifest) => app_core::Event::UpdateChecked { manifest },
        Err(reason) => app_core::Event::UpdateFailed { reason },
    }
}

async fn check_with(
    https: &Https,
    running: &str,
    keys: &[&str],
) -> Result<app_core::UpdateManifest, String> {
    if keys.is_empty() {
        return Err(app_io::update::UpdateError::NoKey.to_string());
    }
    let manifest = https
        .get_bytes(MANIFEST_URL, app_io::update::MAX_MANIFEST_BYTES)
        .await?;
    let signature = https.get_bytes(SIGNATURE_URL, 4096).await?;
    let signature =
        String::from_utf8(signature).map_err(|_| "the signature is not text".to_owned())?;
    app_io::update::verify_manifest(&manifest, &signature, keys, running).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn only_github_release_hosts() {
        assert!(allowed_url("https://github.com/dariy/dail/releases/latest/download/m").is_ok());
        assert!(allowed_url("https://release-assets.githubusercontent.com/x?sig=1").is_ok());
        assert!(allowed_url("https://objects.githubusercontent.com/x").is_ok());
        assert!(allowed_url("http://github.com/x").is_err());
        assert!(allowed_url("https://github.com.evil.org/x").is_err());
        assert!(allowed_url("https://evil.org/github.com").is_err());
        assert!(allowed_url("https://user@github.com/x").is_err());
    }

    #[test]
    fn redirects_resolve_against_the_host() {
        assert_eq!(
            redirect_target("github.com", "/a/b"),
            "https://github.com/a/b"
        );
        assert_eq!(
            redirect_target("github.com", "https://objects.githubusercontent.com/x"),
            "https://objects.githubusercontent.com/x"
        );
        // A scheme-relative location is not a path: it is refused as a URL.
        assert!(allowed_url(&redirect_target("github.com", "//evil.org/x")).is_err());
    }

    #[tokio::test]
    async fn a_get_sends_only_the_version() {
        let (client, mut server) = tokio::io::duplex(4096);
        let serve = tokio::spawn(async move {
            let mut request = vec![0_u8; 1024];
            let n = server.read(&mut request).await.unwrap();
            server
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello")
                .await
                .unwrap();
            String::from_utf8_lossy(&request[..n]).into_owned()
        });
        let response = get_on(client, "github.com", "/m.json", "dail/0.1.0")
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let mut body = response.into_body();
        let chunk = next_chunk(&mut body).await.unwrap().unwrap();
        assert_eq!(&chunk[..], b"hello");
        let request = serve.await.unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /m.json http/1.1\r\n"), "{request}");
        assert!(request.contains("user-agent: dail/0.1.0\r\n"), "{request}");
        assert!(request.contains("host: github.com\r\n"), "{request}");
        assert!(!request.contains("cookie"), "{request}");
    }

    /// The live release page answers over app-tls (ADR 0011: a TLS change at GitHub shows here).
    #[tokio::test]
    #[ignore = "network: set DAIL_IT_UPDATES=1"]
    async fn github_answers_over_app_tls() {
        if std::env::var("DAIL_IT_UPDATES").as_deref() != Ok("1") {
            return;
        }
        crate::tls::install_default_provider();
        let https = Https::new(crate::tls::client_config().unwrap(), "0.0.0");
        let response = https.open("https://github.com/dariy/dail/releases").await;
        assert!(response.is_ok(), "{:?}", response.err());
    }
}
