//! Certificates a user may trust for an account, or has trusted (ADR 0016): what the palette's
//! steps, the problem's *Trust…* confirmation and the account settings show of them. Subject and
//! issuer come from the server: they are cleaned like any untrusted text.

use app_core::{AccountId, AppState, TrustedCert};

use crate::format;
use crate::i18n::{Key, Lang, tr};

/// A fingerprint as `AB:CD:…`, the way browsers and `openssl x509 -fingerprint` show it.
pub fn colons(sha256: &str) -> String {
    sha256
        .as_bytes()
        .chunks(2)
        .map(|pair| String::from_utf8_lossy(pair).to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join(":")
}

/// The certificate's subject (its title), or its fingerprint when it has none.
pub fn title(cert: &TrustedCert) -> String {
    let subject = crate::text::clean(&cert.subject);
    if subject.trim().is_empty() {
        colons(&cert.sha256)
    } else {
        subject
    }
}

/// Issuer, validity and fingerprint, one per line.
pub fn detail(cert: &TrustedCert, lang: Lang) -> String {
    let issuer = crate::text::clean(&cert.issuer);
    let issuer = if cert.issuer == cert.subject {
        tr(lang, Key::CertSelfSigned, &[])
    } else {
        tr(lang, Key::CertIssuer, &[("issuer", &issuer)])
    };
    let dates = tr(
        lang,
        Key::CertValidity,
        &[
            ("from", &format::long_date(cert.not_before_ms, format::utc)),
            ("until", &format::long_date(cert.not_after_ms, format::utc)),
        ],
    );
    let sha = tr(
        lang,
        Key::CertFingerprint,
        &[("sha256", &colons(&cert.sha256))],
    );
    format!("{issuer}\n{dates}\n{sha}")
}

/// The certificates offered to `account`, newest first.
pub fn offers(state: &AppState, account: AccountId) -> Vec<&TrustedCert> {
    state
        .status
        .cert_offers
        .iter()
        .rev()
        .filter(|o| o.account == account)
        .map(|o| &o.cert)
        .collect()
}

/// Every offer, newest first, with its account's address: `(account, address, certificate)`.
pub fn all_offers(state: &AppState) -> Vec<(AccountId, String, &TrustedCert)> {
    state
        .status
        .cert_offers
        .iter()
        .rev()
        .filter_map(|o| {
            let account = state.accounts.get(&o.account)?;
            Some((o.account, account.email.clone(), &o.cert))
        })
        .collect()
}

/// Every trusted certificate, with its account's address.
pub fn all_trusted(state: &AppState) -> Vec<(AccountId, String, &TrustedCert)> {
    state
        .accounts
        .values()
        .flat_map(|a| a.trusted_certs.iter().map(|c| (a.id, a.email.clone(), c)))
        .collect()
}
