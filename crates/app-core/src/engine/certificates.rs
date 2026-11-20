//! Per-account certificate trust (ADR 0016): `Event::CertificateOffered` keeps the certificate a
//! server sent for the accounts on that server; `trust_certificate` adds one to an account,
//! `forget_certificate` removes it (and offers it again, so that undo and redo work).

use crate::account::TrustedCert;
use crate::command::Command;
use crate::error::CoreError;
use crate::event::Effect;
use crate::ids::AccountId;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::state::CertOffer;

use super::{Applied, Engine, invalid, not_found};

/// The most offers kept, oldest dropped first.
pub const MAX_CERT_OFFERS: usize = 16;

/// `sha256` as the engine compares it: lower-case hex without colons or spaces.
fn fingerprint(sha256: &str) -> Result<String, CoreError> {
    sha256_hex("sha256", sha256)
}

/// A SHA-256 fingerprint as the engine compares it (lower-case hex without colons or spaces),
/// or `error_002_invalid_argument` on `field`.
pub(super) fn sha256_hex(field: &str, sha256: &str) -> Result<String, CoreError> {
    let hex: String = sha256
        .chars()
        .filter(|c| !matches!(c, ':' | ' '))
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(hex)
    } else {
        Err(invalid(
            field,
            "expected a SHA-256 fingerprint: 64 hex digits",
        ))
    }
}

impl Engine {
    /// `Event::CertificateOffered`: the offer, for every account with a server at `host:port`.
    pub(super) fn ingest_certificate_offered(
        &mut self,
        host: &str,
        port: u16,
        cert: TrustedCert,
        receipt: CommandReceipt,
    ) -> Outcome {
        let Ok(sha256) = fingerprint(&cert.sha256) else {
            return Outcome::new(receipt);
        };
        let cert = TrustedCert { sha256, ..cert };
        let accounts: Vec<AccountId> = self
            .state
            .accounts
            .values()
            .filter(|a| {
                let at = |e: &crate::account::Endpoint| {
                    e.port == port && e.host.eq_ignore_ascii_case(host)
                };
                at(&a.imap) || at(&a.smtp) || a.managesieve().is_some_and(|m| at(&m))
            })
            .filter(|a| !a.trusted_certs.iter().any(|t| t.sha256 == cert.sha256))
            .map(|a| a.id)
            .collect();
        let mut receipt = receipt;
        for account in accounts {
            self.offer(account, cert.clone());
            receipt = receipt.affecting(EntityRef::Account(account));
        }
        Outcome::new(receipt.affecting(EntityRef::Status))
    }

    /// Keeps `cert` as an offer for `account`, replacing an equal one, newest last.
    fn offer(&mut self, account: AccountId, cert: TrustedCert) {
        let offers = &mut self.state.status.cert_offers;
        offers.retain(|o| !(o.account == account && o.cert.sha256 == cert.sha256));
        offers.push(CertOffer { account, cert });
        if offers.len() > MAX_CERT_OFFERS {
            let excess = offers.len() - MAX_CERT_OFFERS;
            offers.drain(..excess);
        }
    }

    pub(super) fn trust_certificate(
        &mut self,
        account: AccountId,
        sha256: &str,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let sha256 = fingerprint(sha256)?;
        let entry = self
            .state
            .accounts
            .get(&account)
            .ok_or_else(|| not_found("account", account))?;
        if entry.trusted_certs.iter().any(|c| c.sha256 == sha256) {
            return Err(invalid(
                "sha256",
                "the account trusts this certificate already",
            ));
        }
        let at = self
            .state
            .status
            .cert_offers
            .iter()
            .position(|o| o.account == account && o.cert.sha256 == sha256)
            .ok_or_else(|| not_found("certificate offer", &sha256))?;
        let offer = self.state.status.cert_offers.remove(at);
        let entry = self
            .state
            .accounts
            .get_mut(&account)
            .ok_or_else(|| not_found("account", account))?;
        entry.trusted_certs.push(offer.cert);
        let mut applied = self.certificate_changed(
            account,
            &sha256,
            receipt,
            "Trust the certificate",
            Command::ForgetCertificate {
                account,
                sha256: sha256.clone(),
            },
        );
        // Test the login again, now with the certificate.
        if let Some(entry) = self.state.accounts.get(&account) {
            applied.outcome = applied.outcome.with_effect(Effect::VerifyAccount {
                account,
                login: entry.email.clone(),
                imap: entry.imap.clone(),
                smtp: entry.smtp.clone(),
                secret: entry.secret.clone(),
                trusted: entry.imap_login().trusted,
            });
        }
        Ok(applied)
    }

    pub(super) fn forget_certificate(
        &mut self,
        account: AccountId,
        sha256: &str,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let sha256 = fingerprint(sha256)?;
        let entry = self
            .state
            .accounts
            .get_mut(&account)
            .ok_or_else(|| not_found("account", account))?;
        let at = entry
            .trusted_certs
            .iter()
            .position(|c| c.sha256 == sha256)
            .ok_or_else(|| not_found("trusted certificate", &sha256))?;
        let cert = entry.trusted_certs.remove(at);
        // Offered again, so that undo (and the user) can trust it back.
        self.offer(account, cert);
        Ok(self.certificate_changed(
            account,
            &sha256,
            receipt,
            "Forget the certificate",
            Command::TrustCertificate {
                account,
                sha256: sha256.clone(),
            },
        ))
    }

    fn certificate_changed(
        &self,
        account: AccountId,
        sha256: &str,
        receipt: CommandReceipt,
        label: &str,
        inverse: Command,
    ) -> Applied {
        let mut outcome = Outcome::new(
            receipt
                .affecting(EntityRef::Account(account))
                .affecting(EntityRef::Status)
                .with_data(serde_json::json!({ "account": account, "sha256": sha256 })),
        );
        if let Some(entry) = self.state.accounts.get(&account) {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: crate::event::PersistDelta::Account(entry.clone()),
            });
        }
        Applied {
            outcome,
            undo: Some((label.to_owned(), vec![inverse])),
        }
    }
}
