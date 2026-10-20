# 0006 — Finding a domain's mail servers over HTTPS (autoconfig)

- Status: accepted
- Date: 2026-10-19
- Plan: `ROADMAP.md`, Wave 2 (UX & platform)

## Context

Adding an account of an unknown provider (`custom`) asks for the IMAP and SMTP servers, which most people don't know.
Thunderbird finds them from the address: the provider's own autoconfig file
(`https://autoconfig.<domain>/mail/config-v1.1.xml`, then `https://<domain>/.well-known/autoconfig/…`) and, failing
that, Mozilla's database of providers (ISPDB, `https://autoconfig.thunderbird.net/v1.1/<domain>`). DNS SRV records
(RFC 6186) are another source; they need a DNS resolver, which dail does not have.

Until now dail talks only to the account's own servers. Asking a provider's web server, or Mozilla, is a new kind of
connection: it tells them which domain is being set up (not the full address: dail leaves the `emailaddress` parameter
out), before the user has entered a password.

## Decision

- `discover_servers {email}` (a Query, allowed for agents: it reveals only public data) emits
  `Effect::DiscoverServers{email, domain}`. The runtime (`app_sync::autoconfig`) tries the three HTTPS URLs in order,
  each with a 10 s limit, a 64 KiB body limit and at most three HTTPS redirects, over the same rustls + app-tls client
  as IMAP (`tls::client_config`): no new crates, and no plain HTTP, so the answer can't be forged on the way.
- The XML is read by a small parser (`app_io::autoconfig`): the first `incomingServer type="imap"` and
  `outgoingServer type="smtp"` with `SSL` or `STARTTLS`, placeholders such as `%EMAILDOMAIN%` filled in. Servers that
  offer only plaintext are not used.
- The result is a value in `status.discovery`; nothing is configured by it. The account wizards pre-fill the server
  fields, the user (or `add_account`'s caller) still decides.
- SRV records stay a roadmap item: they need a DNS resolver (a new dependency class with its own ADR).

## Consequences

- Most hosted domains get their servers filled in.
- The provider's web server, or Mozilla, learns that the domain is being set up, from the user's IP address. The wizard
  says where it looks.
- app-tls offers ChaCha20-Poly1305 with X25519 only (`docs/plan-tls.md`); a web server without them fails the lookup,
  and the user types the servers as before.
