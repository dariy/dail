# 0014 — app-tls takes its primitives from crates (amends 0003)

- Status: accepted
- Date: 2026-11-03
- Plan: `ROADMAP.md`, Wave 11, STORE-11; `docs/reviews/2026-11-02-store-readiness.md` §7

## Context

ADR 0003 made `app-tls` a rustls provider written for dail, about 1,720 code lines, of which about 1,100 were
hand-written primitives: ChaCha20, Poly1305, X25519, P-256/P-384 arithmetic with ECDSA, RSA with its big integers and a
DER reader. It promised an outside review before a public release. Releases became public in Wave 7 without one, and a
store launch puts dail in front of many more users.

Tier 1 also left out AES-GCM and the NIST key exchange groups, which RFC 8446 §9.1 makes mandatory to implement. Without
them Microsoft 365, T-Online and Namecheap Private Email do not connect.

ADR 0003 weighed only the packaged provider `rustls-rustcrypto`, not the separate crates behind dail's own glue. The
crates were dev-dependencies, used as test oracles, and `rsa` was kept out over RUSTSEC-2023-0071.

## Decision

- `app-tls` keeps only the rustls glue: cipher suites, record protection (nonces, additional data, the explicit nonce of
  TLS 1.2 GCM), key exchange groups (key shares, point encoding, the checks of RFC 8446 §7.4), signature verification
  (the algorithm table for webpki, key limits), hashes and randomness. It stays in safe Rust.
- The primitives come from crates, and each one's audit status is recorded here (as checked on 2026-11-02):

  | Need | Crate | Independent audit |
  |---|---|---|
  | ChaCha20-Poly1305 | `chacha20poly1305` 0.11 | NCC Group, 2020 (an older version) |
  | AES-128/256-GCM | `aes-gcm` 0.11 | NCC Group, 2020 (an older version) |
  | X25519 | `x25519-dalek` 3 | None stated |
  | P-256, P-384 (ECDH, ECDSA verification) | `p256`, `p384` 0.14, `ecdsa` 0.17 | None: "never been independently audited" |
  | RSA verification | `rsa` 0.9 (the stable line) | Include Security; one minor finding, fixed |
  | SHA-2 | `sha2` 0.11 | None stated |

- The provider gains AES-256-GCM and AES-128-GCM for TLS 1.3 and TLS 1.2 (ECDHE with ECDSA or RSA), after
  ChaCha20-Poly1305, and P-256 and P-384 key exchange after X25519. Nine suites, three groups; the server chooses.
- `rsa` carries RUSTSEC-2023-0071 (Marvin), unpatched in 0.9. It is a timing side channel on private-key operations;
  `app-tls` loads no private keys and only verifies signatures, so `deny.toml` ignores the advisory with that reason.
  0.10 is still a release candidate.
- ADR 0003's dependency rule for `app-tls` widens to these crates, all pure Rust. AGENTS.md §2 follows.
- ADR 0003's review promise is retired: no hand-written primitive remains. About 600 lines of glue stay unreviewed.
- The Wycheproof and RFC vectors and the in-memory handshake tests stay and run against the crates through the glue,
  and a new test runs every suite with every group. `live_it::tls_reaches_the_provider_list` probes the provider list
  through `app-tls`; ARCHITECTURE §5.3 records its results.

## Consequences

- Microsoft 365, T-Online, Namecheap Private Email and mail.ru connect; Microsoft 365 still needs OAuth2 to log in.
- dail's own cryptographic code drops from about 1,720 lines to about 600 lines of glue. What remains unaudited is
  mostly the curves (`p256`, `p384`, `ecdsa`), which have far more users than a hand-written version would.
- These crates use `unsafe` for CPU acceleration (AES-NI, ARMv8 crypto extensions), so ADR 0003's "safe Rust" now
  covers the glue only.
- An upgrade of rustls or of one of these crates updates the glue and its tests together. `rsa` moves to 0.10 once it
  is released; that removes the advisory ignore.
