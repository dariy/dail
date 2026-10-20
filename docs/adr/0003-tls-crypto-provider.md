# 0003 — A TLS crypto provider written for dail replaces ring

- Status: accepted
- Date: 2026-10-18
- Plan: `docs/plan-tls.md`

## Context

`AGENTS.md §5` says that no C is compiled or linked at build time. rustls's `ring` provider was the one exception
(`deny.toml`): it compiles C and assembly for the target, so every build needs a C compiler, and `cfg(windows)` code can't
even be checked on Linux without a C cross compiler.

The general-purpose alternatives don't fit:

| Option | Why not |
|---|---|
| `ring` (today) | Compiles C and assembly; no Windows check from Linux |
| `rustls-graviola` 0.4 | Needs ADX (plus AVX2, BMI2, AES-NI) and has no fallback. x86 CPUs before Broadwell (2014), the dev machine among them, can't run it |
| `rustls-rustcrypto` 0.0.2-alpha | Unaudited alpha; no release since 2024-04 |
| aws-lc-rs, BoringSSL, OpenSSL, mbedTLS, SymCrypt, wolfSSL providers | C |
| `oxitls`, IronCrypto (`ic-*`) | Unknown and unvetted |

dail needs much less than these libraries cover. It is a TLS client only: no server role, no client certificates.
Signatures are only verified, so RSA and ECDSA only see public data; that code must be exact, but need not be
constant-time. The secrets are short-lived (one X25519 key per handshake, the session keys), and X25519 and
ChaCha20-Poly1305 are designed to be constant-time in plain integer code.

Measured on 2026-10-18 with handshakes only (`docs/plan-tls.md` §2, Appendix A): Gmail and Hostinger accept TLS 1.3 and
TLS 1.2 with ChaCha20-Poly1305 and X25519 on IMAP 993, SMTP 465 and SMTP 587. Their chains use ECDSA P-256/P-384 with
SHA-256/384 (Gmail) and RSA 2048–4096 with PKCS#1 v1.5 and PSS (Hostinger). So do Yahoo, iCloud, Fastmail, Zoho, GMX,
AOL, Yandex, IONOS, one.com, Migadu, web.de, mailbox.org, Purelymail and mail.ru. Namecheap Private Email needs AES-GCM;
Microsoft 365 and T-Online need AES-GCM with P-256/P-384 key exchange.

## Decision

- A new crate, `app-tls`, provides a rustls `CryptoProvider` with only what dail's TLS client needs, in safe Rust.
  rustls keeps the protocol; `rustls-webpki` keeps certificate parsing and path building.
- Tier 1, in scope:
  - TLS 1.3 and TLS 1.2; the suites `TLS13_CHACHA20_POLY1305_SHA256`, `TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256`
    and `TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256`;
  - X25519 key exchange only;
  - signature verification: ECDSA P-256/P-384 with SHA-256/384; RSA PKCS#1 v1.5 and PSS with SHA-256/384/512 and
    2048–8192-bit keys;
  - SHA-256 for the suites (from `sha2`, already in the tree), HMAC-SHA256, rustls's HKDF and TLS 1.2 PRF over it;
  - random numbers from the OS through `getrandom`.
- Out of scope: AES-GCM and P-256/P-384 key exchange (tiers 2 and 3, with the Microsoft 365 / OAuth2 item), Ed25519
  and P-521 certificates, client certificates, the server side, QUIC, kTLS key extraction, FIPS.
- Dependency edges: `app-tls ← (nothing internal)` and `app-sync ← app-core, app-io, app-tls`. `app-tls` may depend
  only on `rustls` (no provider features), `sha2`, `getrandom` and `zeroize`.
- The code that handles secrets doesn't branch on them, index with them or divide by them, and zeroizes keys on drop.
  Every parser and verifier refuses malformed input with an error, never a panic. Wycheproof and RFC vectors, and
  property tests against RustCrypto crates (dev-dependencies only), check every primitive.
- `ring` is banned in `deny.toml` once app-sync uses the new provider.

## Consequences

- No crate in the build compiles C any more, and `cfg(windows)` code can be checked on Linux.
- A server that offers none of the tier 1 algorithms fails the handshake. dail then says so ("the server supports none
  of the TLS ciphers dail implements") instead of showing a bare alert. Microsoft 365, T-Online and Namecheap Private
  Email don't work until tiers 2 and 3 land.
- dail now owns about 1,900 lines of cryptographic code that no outside party has reviewed. The scope is kept small,
  every primitive is tested against published vectors, and an outside review is needed before a public release.
- The rustls glue (`suites`, `aead`, `verify`, `hash`) follows rustls 0.23; an upgrade to 0.24 updates it together
  with its tests.
