# 0016 — Certificate trust: system roots, user CAs and per-account trust

- Status: accepted
- Date: 2026-11-19
- Plan: `ROADMAP.md`, *After launch* item 2 (CA-1 to CA-3)

## Context

dail trusts exactly the Mozilla roots of `webpki-roots` (ARCHITECTURE §5.3, §9). This fails in three cases:

1. A company or school installs its own root in the OS store (a TLS-inspecting proxy, an internal mail server). The
   browser trusts the server; dail does not.
2. On Android, the user installs a CA under *Settings → Security → Encryption & credentials*. Apps see it in the
   `AndroidCAStore` key store; dail does not read it.
3. A self-hoster runs Dovecot and Postfix with a self-signed certificate or one from a private CA. No store trusts it,
   and there is, rightly, no "accept invalid certificates" switch.

Constraints: no C compiled (AGENTS.md §5), no option that turns verification off, an agent must never be able to widen
trust (AGENTS.md §4, agent policy), and `app-core` stays sans-IO (it cannot parse X.509).

## Decision

- **Roots (CA-1).** A connection trusts the union of: the Mozilla roots; on the desktop, the OS store read once per
  process by `rustls-native-certs` (pure Rust: PEM files found by `openssl-probe` on Linux, `schannel` on Windows,
  `security-framework` on macOS; a store that fails to load is logged and skipped); on Android, the certificates Kotlin
  reads from `AndroidCAStore` (system and user entries) and passes as hex DER in `open`'s `Config.ca_certs`. iOS gives
  apps no way to list its trust store, so iOS keeps the Mozilla roots (and per-account trust). One set, built once in
  `app_sync::tls`, serves IMAP, SMTP (lettre gets the roots one by one through `add_root_certificate`), ManageSieve,
  autoconfig and the update check.
- **Per-account trust (CA-2).** When a handshake fails with `UnknownIssuer`, the runtime's verifier keeps the *top* of
  the chain the server sent: the self-signed certificate itself, or the last certificate of a private chain (its CA, or
  the highest intermediate sent). The runtime reports it as `Event::CertificateOffered { host, port, cert }` with the
  certificate as hex DER, its SHA-256, subject, issuer and validity, read in `app-sync`. The engine keeps the offer, as
  transient status, for every account that uses that host and port. `trust_certificate { account, sha256 }` moves the
  pending offer with that fingerprint into `Account.trusted_certs` (saved in `config.toml`); the fingerprint must match
  an offer, so no certificate can be supplied through arguments. `forget_certificate { account, sha256 }` removes one.
  Both are undoable mutations. The trusted certificates travel in the account's `ImapLogin` and `SmtpLogin` and are
  added as extra trust anchors for that account's connections only.
- **Still checked.** A trusted certificate is a trust anchor, not an exception: the host name, the validity dates and
  the signatures are verified as for any other chain. A server whose certificate does not name its host, or has
  expired, still fails, and no fix is offered.
- **Agents.** `trust_certificate` is `user_only` (it decides whom dail talks to). `forget_certificate` only narrows
  trust and is `allowed`.
- **SMTP.** lettre builds its own verifier, so it cannot report the chain. When an SMTP handshake fails on the
  certificate, the runtime probes the server once with its own verifier (TLS, or `EHLO` + `STARTTLS`) to make the offer.

## Consequences

- Corporate and Android user CAs work without any setting; a self-hoster trusts their server once, from the error.
- The trust in a CA from the OS store is the OS's decision; dail cannot show which store a root came from.
- A server that changes to a new self-signed certificate fails again and offers the new one: that is the point.
- `config.toml` gains `[[account.trusted_cert]]` (`der`, `sha256`, `subject`), absent from older files and golden
  profiles; `ImapLogin` and `SmtpLogin` gain `trusted` (omitted when empty). Both additions are backwards compatible.
- One new dependency, `rustls-native-certs`, on the desktop targets only.
