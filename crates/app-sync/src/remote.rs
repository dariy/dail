//! The device side of a dail server (ADR 0017): the TLS dialer that pins the server's
//! certificate, the probe that reads its fingerprint, pairing with a code, and
//! [`RemoteClient`], the protocol of [`crate::serve`] (`hello`, `submit`, `state`, `watch`,
//! `blob`, `put_file`).
//!
//! The server's certificate is accepted only if its SHA-256 equals the pinned fingerprint: web
//! roots, names and dates are never consulted. The handshake's signature is still verified with
//! the provider's algorithms, so the server must hold the certificate's key.

use std::sync::Arc;
use std::time::Duration;

use app_core::diff::StateDiff;
use app_core::{Command, DeviceId, DeviceRole, Origin, RequestId, Secret};
use base64ct::{Base64, Encoding as _};
use parking_lot::Mutex;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt as _;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use crate::error::SyncError;
use crate::ipc::{PreviewReply, SubmitReply, wire_command};
use crate::rpc::{CallError, Notifications, RpcClient};
use crate::serve::PROTOCOL;
use crate::session::{PreviewResult, SubmitResult};

/// How long connecting, the TLS handshake and `hello` may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Where the verifier leaves the fingerprint the server presented.
type SeenFingerprint = Arc<Mutex<Option<String>>>;

/// Accepts only the pinned certificate; without a pin (a probe) accepts any and records it.
#[derive(Debug)]
struct PinnedVerifier {
    pinned: Option<String>,
    seen: SeenFingerprint,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fingerprint = crate::trust::fingerprint(end_entity);
        let accepted = self
            .pinned
            .as_ref()
            .is_none_or(|pinned| pinned.eq_ignore_ascii_case(&fingerprint));
        *self.seen.lock() = Some(fingerprint);
        if accepted {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "the server's certificate is not the pinned one".to_owned(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn remote_error(what: &str, err: impl std::fmt::Display) -> SyncError {
    SyncError::Remote(format!("{what}: {err}"))
}

/// A client config for a dail server: TLS 1.3, the app-tls provider, and a verifier that
/// accepts only `pinned` (any certificate when `None`, which only a probe uses). The second
/// value receives the fingerprint the server presented.
fn client_config(pinned: Option<&str>) -> Result<(Arc<ClientConfig>, SeenFingerprint), SyncError> {
    let provider = Arc::new(app_tls::provider());
    let seen = Arc::new(Mutex::new(None));
    let verifier = PinnedVerifier {
        pinned: pinned.map(str::to_ascii_lowercase),
        seen: Arc::clone(&seen),
        provider: Arc::clone(&provider),
    };
    let config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|err| SyncError::Tls(err.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok((Arc::new(config), seen))
}

/// The name sent in the handshake: the host if it is a DNS name or an IP address. The server
/// does not look at it and the device checks only the pin.
fn server_name(host: &str) -> ServerName<'static> {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    ServerName::try_from(host.to_owned())
        .unwrap_or(ServerName::IpAddress(std::net::Ipv4Addr::LOCALHOST.into()))
}

/// Opens a TLS connection to `host:port`; see [`client_config`].
async fn dial(
    host: &str,
    port: u16,
    pinned: Option<&str>,
) -> Result<(TlsStream<TcpStream>, String), SyncError> {
    let (config, seen) = client_config(pinned)?;
    let address = format!("{host}:{port}");
    let connect = async {
        let tcp = TcpStream::connect((host.trim_start_matches('[').trim_end_matches(']'), port))
            .await
            .map_err(|err| remote_error(&format!("cannot reach {address}"), err))?;
        let _ = tcp.set_nodelay(true);
        TlsConnector::from(config)
            .connect(server_name(host), tcp)
            .await
            .map_err(|err| remote_error(&format!("TLS with {address} failed"), err))
    };
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, connect)
        .await
        .map_err(|_| SyncError::Remote(format!("{address} did not answer in time")))??;
    let fingerprint = seen
        .lock()
        .clone()
        .ok_or_else(|| SyncError::Remote(format!("{address} presented no certificate")))?;
    Ok((stream, fingerprint))
}

/// Connects to `host:port` without trusting it and returns the SHA-256 fingerprint of the
/// certificate it presents (lower-case hex), for the user to compare (`Event::ServerOffered`).
/// Nothing is sent over the connection.
pub async fn probe(host: &str, port: u16) -> Result<String, SyncError> {
    let (mut stream, fingerprint) = dial(host, port, None).await?;
    let _ = stream.shutdown().await;
    Ok(fingerprint)
}

/// A raw protocol connection to the server at `host:port` that accepts only the certificate
/// with `fingerprint`, before `pair` or `hello` (tools and tests; devices use [`pair`] and
/// [`RemoteClient`]).
pub async fn open_rpc(host: &str, port: u16, fingerprint: &str) -> Result<RpcClient, SyncError> {
    let (stream, _) = dial(host, port, Some(fingerprint)).await?;
    Ok(RpcClient::new(stream))
}

/// A device paired with a server ([`pair`]).
#[derive(Debug)]
pub struct Paired {
    /// The device token, for the OS keyring only.
    pub token: Secret,
    /// The device's ID on the server.
    pub device: DeviceId,
    /// What it may do.
    pub role: DeviceRole,
}

#[derive(Deserialize)]
struct PairAnswer {
    token: String,
    device: DeviceInfo,
}

/// A device as `pair` and `hello` describe it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeviceInfo {
    /// Its ID on the server.
    pub id: DeviceId,
    /// Its name.
    pub name: String,
    /// What it may do.
    pub role: DeviceRole,
}

fn call_error(method: &str, err: CallError) -> SyncError {
    match err {
        CallError::Closed => {
            SyncError::Remote(format!("the server closed the connection ({method})"))
        }
        CallError::Protocol(message) => SyncError::Remote(format!("{method}: {message}")),
        CallError::Refused { message, .. } => {
            SyncError::Remote(format!("{method} refused: {message}"))
        }
    }
}

/// Exchanges a pairing `code` for a device token with the server at `host:port`, accepting only
/// the certificate with `fingerprint`.
pub async fn pair(
    host: &str,
    port: u16,
    fingerprint: &str,
    code: &Secret,
) -> Result<Paired, SyncError> {
    let rpc = open_rpc(host, port, fingerprint).await?;
    let call = rpc.call("pair", json!({ "code": code.expose() }));
    let answer = tokio::time::timeout(CONNECT_TIMEOUT, call)
        .await
        .map_err(|_| SyncError::Remote("the server did not answer the pairing".to_owned()))?
        .map_err(|err| call_error("pair", err))?;
    let answer: PairAnswer =
        serde_json::from_value(answer).map_err(|err| remote_error("bad pair answer", err))?;
    Ok(Paired {
        token: Secret::new(answer.token),
        device: answer.device.id,
        role: answer.device.role,
    })
}

/// Where a paired device connects, and with what.
#[derive(Debug, Clone)]
pub struct RemoteTarget {
    /// The server's host.
    pub host: String,
    /// Its port.
    pub port: u16,
    /// The pinned SHA-256 fingerprint of its certificate, hex.
    pub fingerprint: String,
    /// The device token.
    pub token: Secret,
}

/// The server's answer to `hello`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Hello {
    /// The protocol version ([`PROTOCOL`]).
    pub protocol: u32,
    /// The server's dail version.
    pub server_version: String,
    /// The server process: another value than last time means it restarted, and its revisions
    /// started over.
    pub instance: String,
    /// This device.
    pub device: DeviceInfo,
}

/// A change of the server's state, from [`RemoteWatch::next`].
#[derive(Debug, Clone, PartialEq)]
pub enum RemoteChange {
    /// The change since the last one.
    Changed {
        /// The server's revision after it.
        revision: u64,
        /// The change.
        diff: Box<StateDiff>,
    },
    /// The whole shared state: the device was too far behind, or the server restarted.
    Resync {
        /// The server's revision.
        revision: u64,
        /// A snapshot (a diff from an empty state).
        state: Box<StateDiff>,
    },
}

/// The notifications of `watch`.
#[derive(Debug)]
pub struct RemoteWatch {
    notifications: Notifications,
}

#[derive(Deserialize)]
struct ChangedParams {
    revision: u64,
    diff: Box<StateDiff>,
}

#[derive(Deserialize)]
struct ResyncParams {
    revision: u64,
    state: Box<StateDiff>,
}

impl RemoteWatch {
    /// The next change; `None` once the connection is closed. Cancel-safe.
    pub async fn next(&mut self) -> Option<RemoteChange> {
        loop {
            let (method, params) = self.notifications.next().await?;
            let change = match method.as_str() {
                "changed" => serde_json::from_value::<ChangedParams>(params)
                    .ok()
                    .map(|p| RemoteChange::Changed {
                        revision: p.revision,
                        diff: p.diff,
                    }),
                "resync" => serde_json::from_value::<ResyncParams>(params)
                    .ok()
                    .map(|p| RemoteChange::Resync {
                        revision: p.revision,
                        state: p.state,
                    }),
                _ => None,
            };
            match change {
                Some(change) => return Some(change),
                None => tracing::debug!(method, "notification skipped"),
            }
        }
    }
}

/// A connection of a paired device to its dail server, after `hello`. Cheap to clone; clones
/// share the connection. Calls are answered in order, one at a time.
#[derive(Debug, Clone)]
pub struct RemoteClient {
    target: RemoteTarget,
    rpc: RpcClient,
    hello: Hello,
}

/// Dials `target` and says `hello`.
async fn connect_rpc(target: &RemoteTarget) -> Result<(RpcClient, Hello), SyncError> {
    let (stream, _) = dial(&target.host, target.port, Some(&target.fingerprint)).await?;
    let rpc = RpcClient::new(stream);
    let params = json!({
        "token": target.token.expose(),
        "protocol": PROTOCOL,
        "client_version": env!("CARGO_PKG_VERSION"),
    });
    let answer = tokio::time::timeout(CONNECT_TIMEOUT, rpc.call("hello", params))
        .await
        .map_err(|_| SyncError::Remote("the server did not answer hello".to_owned()))?
        .map_err(|err| call_error("hello", err))?;
    let hello: Hello =
        serde_json::from_value(answer).map_err(|err| remote_error("bad hello answer", err))?;
    if hello.protocol != PROTOCOL {
        return Err(SyncError::Remote(format!(
            "the server speaks protocol {}, not {PROTOCOL}",
            hello.protocol
        )));
    }
    Ok((rpc, hello))
}

impl RemoteClient {
    /// Connects to `target` and says `hello`.
    pub async fn connect(target: RemoteTarget) -> Result<Self, SyncError> {
        let (rpc, hello) = connect_rpc(&target).await?;
        Ok(Self { target, rpc, hello })
    }

    /// Another connection to the same server, for requests that must not wait behind this
    /// one's (pre-loading bodies).
    pub async fn fork(&self) -> Result<Self, SyncError> {
        Self::connect(self.target.clone()).await
    }

    /// The server's answer to `hello`.
    pub const fn hello(&self) -> &Hello {
        &self.hello
    }

    /// A request on the connection. A refusal is [`SyncError::Refused`] (the connection is
    /// fine); a lost or garbled connection is [`SyncError::Remote`].
    async fn call(&self, method: &str, params: Value) -> Result<Value, SyncError> {
        // A body can be as large as the server's own requests may be (`put_file`, a blob in
        // base64); a snapshot as large as `rpc::SNAPSHOT_LIMIT`; anything else is a receipt.
        let limit = match method {
            "blob" => crate::serve::MAX_LINE,
            "state" => crate::rpc::SNAPSHOT_LIMIT,
            _ => crate::rpc::RESPONSE_LIMIT,
        };
        self.rpc
            .call_limited(method, params, limit)
            .await
            .map_err(|err| match err {
                CallError::Refused { code, message } => SyncError::Refused {
                    method: method.to_owned(),
                    code,
                    message,
                },
                err => call_error(method, err),
            })
    }

    /// Submits a command. `request_id` makes a resend harmless (the server answers it from
    /// memory); `seed` is the `Ctx::id_seed` the device used, so the server creates the same
    /// IDs. The outer error is the connection's, the inner one the engine's rejection.
    pub async fn submit(
        &self,
        cmd: &Command,
        origin: Origin,
        request_id: RequestId,
        seed: u128,
    ) -> Result<SubmitResult, SyncError> {
        let params = json!({
            "origin": origin,
            "command": wire_command(cmd)?,
            "request_id": request_id,
            "seed": seed.to_string(),
        });
        let result = self.call("submit", params).await?;
        match serde_json::from_value(result)
            .map_err(|err| remote_error("bad submit answer", err))?
        {
            SubmitReply::Ok(receipt) => Ok(Ok(receipt)),
            SubmitReply::Rejected(err) => Ok(Err(err)),
        }
    }

    /// A dry run on the server.
    pub async fn preview(&self, cmd: &Command, origin: Origin) -> Result<PreviewResult, SyncError> {
        let params = json!({ "origin": origin, "command": wire_command(cmd)? });
        let result = self.call("preview", params).await?;
        match serde_json::from_value(result)
            .map_err(|err| remote_error("bad preview answer", err))?
        {
            PreviewReply::Ok(preview) => Ok(Ok(preview)),
            PreviewReply::Rejected(err) => Ok(Err(err)),
        }
    }

    /// Waits until the server's in-flight effects are done.
    pub async fn flush(&self) -> Result<(), SyncError> {
        self.call("flush", Value::Null).await.map(drop)
    }

    /// The server's revision and a snapshot of its shared state.
    pub async fn state(&self) -> Result<(u64, StateDiff), SyncError> {
        #[derive(Deserialize)]
        struct State {
            revision: u64,
            state: StateDiff,
        }
        let state: State = serde_json::from_value(self.call("state", Value::Null).await?)
            .map_err(|err| remote_error("bad state", err))?;
        Ok((state.revision, state.state))
    }

    /// Watches the server's changes on a connection of its own: first the change since
    /// `since` (a revision of this server instance), or a `resync` if the server does not have
    /// it; then one `changed` after each burst of changes.
    pub async fn watch(&self, since: Option<u64>) -> Result<RemoteWatch, SyncError> {
        let (rpc, _) = connect_rpc(&self.target).await?;
        let params = json!({ "since": since, "instance": self.hello.instance });
        let (_, notifications) = rpc
            .into_notifications("watch", params)
            .await
            .map_err(|err| call_error("watch", err))?;
        Ok(RemoteWatch { notifications })
    }

    /// A stored message body (`blob` of an envelope); `None` if the server has none.
    pub async fn blob(&self, id: &str) -> Result<Option<Vec<u8>>, SyncError> {
        let answer = self.call("blob", json!({ "id": id })).await?;
        if answer.is_null() {
            return Ok(None);
        }
        let bytes = answer["bytes"]
            .as_str()
            .ok_or_else(|| SyncError::Remote("bad blob answer".to_owned()))?;
        Base64::decode_vec(bytes)
            .map(Some)
            .map_err(|err| remote_error("bad blob answer", err))
    }

    /// Uploads a file for `attach_file` and returns its path on the server.
    pub async fn put_file(&self, name: &str, bytes: &[u8]) -> Result<String, SyncError> {
        let params = json!({ "name": name, "bytes": Base64::encode_string(bytes) });
        let answer = self.call("put_file", params).await?;
        answer["path"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| SyncError::Remote("bad put_file answer".to_owned()))
    }
}
