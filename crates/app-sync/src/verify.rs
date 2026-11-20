//! Account verification: IMAP LOGIN and SMTP AUTH over TLS, then disconnect.
//!
//! Both protocols run concurrently. Credentials are only ever sent over an encrypted connection:
//! with `starttls` the upgrade must succeed before login, otherwise verification fails.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use app_core::{Endpoint, Secret};
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use rustls::ClientConfig;

use crate::backend::BoxFuture;
use crate::error::SyncError;
use crate::imap::{self, address};

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
        trusted: &'a [String],
    ) -> BoxFuture<'a, Result<(), String>>;
}

/// Verifies against the real servers.
#[derive(Clone)]
pub struct NetVerifier {
    /// A config for every IMAP connection instead of the account's (tests).
    tls: Option<Arc<ClientConfig>>,
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
    /// Verifies through [`crate::trust`] with the account's certificates, and uses
    /// [`DEFAULT_TIMEOUT`].
    pub fn new() -> Result<Self, SyncError> {
        Ok(Self {
            tls: None,
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
        trusted: &[String],
    ) -> Result<(), String> {
        let tls = imap::config_for(self.tls.as_ref(), endpoint, trusted)?;
        let check = imap_login(&tls, endpoint, login, password);
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
        trusted: &[String],
    ) -> Result<(), String> {
        let check = smtp_auth(endpoint, login, password, trusted);
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
        trusted: &'a [String],
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let (imap, smtp) = tokio::join!(
                self.verify_imap(imap, login, password, trusted),
                self.verify_smtp(smtp, login, password, trusted)
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
    trusted: &[String],
) -> Result<(), String> {
    let mut conn = crate::smtp::connect(endpoint, trusted).await?;
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
