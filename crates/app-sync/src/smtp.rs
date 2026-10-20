//! SMTP submission (`SmtpSend`): TLS (or STARTTLS, required) with the account's password,
//! then `MAIL FROM` / `RCPT TO` for every recipient and the rendered message.

use std::fmt;
use std::time::Duration;

use app_core::{Secret, Security, SmtpLogin};
use lettre::AsyncTransport;
use lettre::address::{Address, Envelope};
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::transport::smtp::client::Tls;
use lettre::{AsyncSmtpTransport, Tokio1Executor};

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

/// Submits to the real server with lettre over rustls (webpki roots). Plaintext is never used:
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
            let tls = tls::smtp_parameters(&login.server.host)
                .map_err(|err| format!("SMTP {at}: TLS setup failed: {err}"))?;
            let tls = match login.server.security {
                Security::Tls => Tls::Wrapper(tls),
                Security::StartTls => Tls::Required(tls),
            };
            let transport: AsyncSmtpTransport<Tokio1Executor> =
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(login.server.host.clone())
                    .port(login.server.port)
                    .tls(tls)
                    .credentials(Credentials::new(
                        login.login.clone(),
                        password.expose().to_owned(),
                    ))
                    .authentication(vec![Mechanism::Plain, Mechanism::Login])
                    .timeout(Some(self.timeout))
                    .build();
            let sent =
                tokio::time::timeout(self.timeout, transport.send_raw(&envelope, message.raw))
                    .await
                    .map_err(|_| format!("SMTP {at}: timed out"))?;
            sent.map(|_| ())
                .map_err(|err| tls::with_hint(format!("SMTP {at}: {err}"), &err))
        })
    }
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
