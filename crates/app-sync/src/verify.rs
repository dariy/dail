//! Account verification: IMAP LOGIN and SMTP AUTH over TLS, then disconnect.
//!
//! Both protocols run concurrently. Credentials are only ever sent over an encrypted connection:
//! with `starttls` the upgrade must succeed before login, otherwise verification fails.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use app_core::{Endpoint, Secret, Security};
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::transport::smtp::client::AsyncSmtpConnection;
use lettre::transport::smtp::extension::ClientId;
use rustls::ClientConfig;

use crate::backend::BoxFuture;
use crate::error::SyncError;
use crate::imap::{self, address};
use crate::tls;

/// Default limit for one protocol check (connect + TLS + login).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Checks that a login works on an account's servers.
pub trait AccountVerifier: Send + Sync + 'static {
    /// `Ok(())` if IMAP LOGIN and SMTP AUTH both succeed; otherwise a diagnostic that never
    /// contains the password.
    fn verify<'a>(
        &'a self,
        login: &'a str,
        password: &'a Secret,
        imap: &'a Endpoint,
        smtp: &'a Endpoint,
    ) -> BoxFuture<'a, Result<(), String>>;
}

/// Verifies against the real servers.
#[derive(Clone)]
pub struct NetVerifier {
    tls: Arc<ClientConfig>,
    timeout: Duration,
}

impl fmt::Debug for NetVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetVerifier")
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl NetVerifier {
    /// Uses the Mozilla root certificates and [`DEFAULT_TIMEOUT`].
    pub fn new() -> Result<Self, SyncError> {
        Ok(Self {
            tls: tls::client_config()?,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    /// Changes the per-protocol timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// IMAP: connect, TLS (or STARTTLS), LOGIN, LOGOUT.
    pub async fn verify_imap(
        &self,
        endpoint: &Endpoint,
        login: &str,
        password: &Secret,
    ) -> Result<(), String> {
        let check = imap_login(&self.tls, endpoint, login, password);
        match tokio::time::timeout(self.timeout, check).await {
            Ok(result) => result,
            Err(_) => Err(format!("IMAP {}: timed out", address(endpoint))),
        }
    }

    /// SMTP: connect, TLS (or STARTTLS), EHLO, AUTH, QUIT.
    pub async fn verify_smtp(
        &self,
        endpoint: &Endpoint,
        login: &str,
        password: &Secret,
    ) -> Result<(), String> {
        let check = smtp_auth(endpoint, login, password, self.timeout);
        match tokio::time::timeout(self.timeout, check).await {
            Ok(result) => result,
            Err(_) => Err(format!("SMTP {}: timed out", address(endpoint))),
        }
    }
}

impl AccountVerifier for NetVerifier {
    fn verify<'a>(
        &'a self,
        login: &'a str,
        password: &'a Secret,
        imap: &'a Endpoint,
        smtp: &'a Endpoint,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let (imap, smtp) = tokio::join!(
                self.verify_imap(imap, login, password),
                self.verify_smtp(smtp, login, password)
            );
            match (imap, smtp) {
                (Ok(()), Ok(())) => Ok(()),
                (Err(err), Ok(())) | (Ok(()), Err(err)) => Err(err),
                (Err(imap), Err(smtp)) => Err(format!("{imap}; {smtp}")),
            }
        })
    }
}

async fn imap_login(
    tls: &Arc<ClientConfig>,
    endpoint: &Endpoint,
    login: &str,
    password: &Secret,
) -> Result<(), String> {
    let mut session = imap::connect_login(tls, endpoint, login, password).await?;
    if let Err(err) = session.logout().await {
        tracing::debug!(error = %err, "IMAP LOGOUT failed after a successful login");
    }
    Ok(())
}

async fn smtp_auth(
    endpoint: &Endpoint,
    login: &str,
    password: &Secret,
    timeout: Duration,
) -> Result<(), String> {
    let at = address(endpoint);
    let tls = tls::smtp_parameters(&endpoint.host)
        .map_err(|err| format!("SMTP {at}: TLS setup failed: {err}"))?;
    let hello = ClientId::default();
    let server = (endpoint.host.as_str(), endpoint.port);
    let mut conn = match endpoint.security {
        Security::Tls => {
            AsyncSmtpConnection::connect_tokio1(server, Some(timeout), &hello, Some(tls), None)
                .await
                .map_err(|err| tls::with_hint(format!("SMTP {at}: connect failed: {err}"), &err))?
        }
        Security::StartTls => {
            let mut conn =
                AsyncSmtpConnection::connect_tokio1(server, Some(timeout), &hello, None, None)
                    .await
                    .map_err(|err| {
                        tls::with_hint(format!("SMTP {at}: connect failed: {err}"), &err)
                    })?;
            conn.starttls(tls, &hello).await.map_err(|err| {
                tls::with_hint(format!("SMTP {at}: STARTTLS failed: {err}"), &err)
            })?;
            conn
        }
    };
    if !conn.is_encrypted() {
        conn.abort().await;
        return Err(format!("SMTP {at}: connection is not encrypted"));
    }
    let credentials = Credentials::new(login.to_owned(), password.expose().to_owned());
    let result = conn
        .auth(&[Mechanism::Plain, Mechanism::Login], &credentials)
        .await;
    match result {
        Ok(_) => {
            if let Err(err) = conn.quit().await {
                tracing::debug!(error = %err, "SMTP QUIT failed after a successful AUTH");
            }
            Ok(())
        }
        Err(err) => {
            conn.abort().await;
            Err(format!("SMTP AUTH failed: {err}"))
        }
    }
}
