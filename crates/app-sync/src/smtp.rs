//! SMTP submission (`SmtpSend`): TLS (or STARTTLS, required) with the account's password,
//! then `MAIL FROM` / `RCPT TO` for every recipient and the rendered message.

use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use app_core::{Endpoint, Secret, Security, SmtpLogin};
use lettre::address::{Address, Envelope};
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::transport::smtp::client::{AsyncSmtpConnection, AsyncTokioStream};
use lettre::transport::smtp::extension::ClientId;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use crate::backend::BoxFuture;
use crate::imap::address;
use crate::tls;

/// Limit for one submission (connect, TLS, AUTH, transfer).
pub const SEND_TIMEOUT: Duration = Duration::from_secs(120);

/// One message to submit.
#[derive(Clone, PartialEq, Eq)]
pub struct Submission<'a> {
    /// `MAIL FROM` address.
    pub from: &'a str,
    /// `RCPT TO` addresses (To, Cc and Bcc).
    pub recipients: &'a [String],
    /// The rendered message (without a Bcc header).
    pub raw: &'a [u8],
}

impl fmt::Debug for Submission<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Submission")
            .field("from", &self.from)
            .field("recipients", &self.recipients)
            .field("bytes", &self.raw.len())
            .finish()
    }
}

/// Submits messages to an SMTP server.
pub trait SmtpSender: Send + Sync + 'static {
    /// `Ok(())` once the server accepted the message for every recipient; otherwise a diagnostic
    /// that never contains the password.
    fn send<'a>(
        &'a self,
        login: &'a SmtpLogin,
        password: &'a Secret,
        message: Submission<'a>,
    ) -> BoxFuture<'a, Result<(), String>>;
}

/// Submits to the real server with lettre over our own TLS ([`connect`]). Plaintext is never used:
/// `tls` connects with TLS, `starttls` requires the upgrade before AUTH.
#[derive(Debug, Clone)]
pub struct NetSmtp {
    timeout: Duration,
}

impl Default for NetSmtp {
    fn default() -> Self {
        Self {
            timeout: SEND_TIMEOUT,
        }
    }
}

impl NetSmtp {
    /// Changes the submission timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl SmtpSender for NetSmtp {
    fn send<'a>(
        &'a self,
        login: &'a SmtpLogin,
        password: &'a Secret,
        message: Submission<'a>,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let at = address(&login.server);
            let envelope = envelope(&message).map_err(|err| format!("SMTP {at}: {err}"))?;
            let submit = async {
                let mut conn = connect(&login.server, &login.trusted).await?;
                let credentials =
                    Credentials::new(login.login.clone(), password.expose().to_owned());
                if let Err(err) = conn
                    .auth(&[Mechanism::Plain, Mechanism::Login], &credentials)
                    .await
                {
                    conn.abort().await;
                    return Err(format!("SMTP {at}: AUTH failed: {err}"));
                }
                let sent = conn.send(&envelope, message.raw).await;
                match sent {
                    Ok(_) => {
                        if let Err(err) = conn.quit().await {
                            tracing::debug!(error = %err, "SMTP QUIT failed after a send");
                        }
                        Ok(())
                    }
                    Err(err) => {
                        conn.abort().await;
                        Err(format!("SMTP {at}: {err}"))
                    }
                }
            };
            tokio::time::timeout(self.timeout, submit)
                .await
                .map_err(|_| format!("SMTP {at}: timed out"))?
        })
    }
}

/// A TLS stream to an SMTP server, verified by [`crate::trust`], as lettre takes one.
#[derive(Debug)]
struct Encrypted {
    /// A greeting to read before the stream's own bytes: after STARTTLS the server sends none,
    /// and lettre reads one before its `EHLO`.
    greeting: &'static [u8],
    stream: TlsStream<TcpStream>,
}

impl AsyncTokioStream for Encrypted {
    fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.get_ref().0.peer_addr()
    }
}

impl AsyncRead for Encrypted {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if !self.greeting.is_empty() {
            let n = self.greeting.len().min(buf.remaining());
            let (now, later) = self.greeting.split_at(n);
            buf.put_slice(now);
            self.greeting = later;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for Encrypted {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

/// Reads one SMTP reply (every line of a multi-line one) byte by byte, so that nothing after it
/// is consumed before the TLS handshake. Returns its code.
async fn reply(tcp: &mut TcpStream, at: &str) -> Result<u16, String> {
    let mut line = Vec::new();
    loop {
        let byte = tcp
            .read_u8()
            .await
            .map_err(|err| format!("SMTP {at}: read failed: {err}"))?;
        line.push(byte);
        if line.len() > 4096 {
            return Err(format!("SMTP {at}: reply line too long"));
        }
        if line.ends_with(b"\r\n") {
            let code = std::str::from_utf8(line.get(..3).unwrap_or_default())
                .ok()
                .and_then(|c| c.parse().ok())
                .ok_or_else(|| format!("SMTP {at}: malformed reply"))?;
            if line.get(3) != Some(&b'-') {
                return Ok(code);
            }
            line.clear();
        }
    }
}

/// Connects to `server` over TLS (or plain, then STARTTLS before anything else) verified by
/// [`crate::trust`] with the account's `trusted` certificates, and greets it (`EHLO`).
pub async fn connect(server: &Endpoint, trusted: &[String]) -> Result<AsyncSmtpConnection, String> {
    let at = address(server);
    let config = crate::trust::account_config(server.port, trusted)
        .map_err(|err| format!("SMTP {at}: TLS setup failed: {err}"))?;
    let name = ServerName::try_from(server.host.clone())
        .map_err(|err| format!("SMTP {at}: invalid host name: {err}"))?;
    let mut tcp = TcpStream::connect((server.host.as_str(), server.port))
        .await
        .map_err(|err| format!("SMTP {at}: connect failed: {err}"))?;
    let greeting: &'static [u8] = match server.security {
        Security::Tls => b"",
        Security::StartTls => {
            let starttls = async {
                if reply(&mut tcp, &at).await? != 220 {
                    return Err(format!("SMTP {at}: refused the connection"));
                }
                let hello = format!("EHLO {}\r\n", ClientId::default());
                tcp.write_all(hello.as_bytes())
                    .await
                    .map_err(|err| format!("SMTP {at}: write failed: {err}"))?;
                if reply(&mut tcp, &at).await? != 250 {
                    return Err(format!("SMTP {at}: EHLO refused"));
                }
                tcp.write_all(b"STARTTLS\r\n")
                    .await
                    .map_err(|err| format!("SMTP {at}: write failed: {err}"))?;
                if reply(&mut tcp, &at).await? != 220 {
                    return Err(format!("SMTP {at}: STARTTLS failed: refused by the server"));
                }
                Ok(())
            };
            starttls.await?;
            b"220 dail\r\n"
        }
    };
    let stream = TlsConnector::from(config)
        .connect(name, tcp)
        .await
        .map_err(|err| tls::with_hint(format!("SMTP {at}: TLS handshake failed: {err}"), &err))?;
    let stream = Box::new(Encrypted { greeting, stream });
    AsyncSmtpConnection::connect_with_transport(stream, &ClientId::default())
        .await
        .map_err(|err| tls::with_hint(format!("SMTP {at}: connect failed: {err}"), &err))
}

/// The SMTP envelope of a submission.
fn envelope(message: &Submission<'_>) -> Result<Envelope, String> {
    let parse = |addr: &str| {
        addr.parse::<Address>()
            .map_err(|err| format!("invalid address {addr}: {err}"))
    };
    let to = message
        .recipients
        .iter()
        .map(|r| parse(r))
        .collect::<Result<Vec<_>, _>>()?;
    Envelope::new(Some(parse(message.from)?), to).map_err(|err| err.to_string())
}

/// Every `To`, `Cc` and `Bcc` address of a message, once (case-insensitive).
pub fn recipients(message: &app_core::OutgoingMessage) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for addr in message.to.iter().chain(&message.cc).chain(&message.bcc) {
        if !out
            .iter()
            .any(|known| known.eq_ignore_ascii_case(&addr.email))
        {
            out.push(addr.email.clone());
        }
    }
    out
}
