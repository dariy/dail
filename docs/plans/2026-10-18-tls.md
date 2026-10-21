# Plan: replace `ring` with a TLS crypto provider written for dail

- Status: implemented on 2026-10-18 (§8; results of the final checks in §8.1). ADR 0003 records §1–§3.
- Goal: no C is compiled anywhere in the build. rustls's `ring` provider is the last crate that compiles C
  (`ARCHITECTURE.md §5.3`, `deny.toml`). Removing it also lets `cfg(windows)` code be checked on Linux.
- Approach: a new crate, `app-tls`, with a rustls `CryptoProvider` that implements only what dail's TLS client needs, in
  safe Rust. rustls keeps the protocol; `rustls-webpki` keeps certificate parsing and path building.
- Assumes the one-binary integration-test layout (`tests/it/main.rs`) of the build-optimization change of 2026-10-18.

## 1. Why not a general-purpose provider

| Option | Why not |
|---|---|
| `ring` (today) | Compiles C and assembly for the target, so every build needs a C compiler for it; no Windows check from Linux |
| `rustls-graviola` 0.4 | Needs ADX (plus AVX2, BMI2, AES-NI): it asserts the features at run time and uses ADX instructions without a fallback. The dev machine (Core i3-4170, Haswell) has no ADX, so TLS would not work there |
| `rustls-rustcrypto` 0.0.2-alpha | Unaudited alpha; no release since 2024-04 |
| aws-lc-rs, BoringSSL, OpenSSL, mbedTLS, SymCrypt, wolfSSL providers | C |
| `oxitls`, IronCrypto (`ic-*`) | Unknown and unvetted |

dail needs much less than these libraries cover:

- It is a TLS client only: no server role, no client certificates. Signatures are only verified, never created, so
  RSA and ECDSA only ever see public data. That code must be exact, but it need not be constant-time.
- The secrets are short-lived: one X25519 key per handshake, and the session keys.
- X25519 and ChaCha20-Poly1305 are designed to be constant-time in plain integer code, and both reference providers
  accept them (§2).

## 2. What the servers need (measured 2026-10-18)

Probed with `openssl s_client`: handshakes only, no login (script in Appendix A). Re-run it before starting.

Reference providers, on every port (IMAP 993, SMTP 465, SMTP 587 with STARTTLS):

| Server | TLS 1.3, ChaCha20-Poly1305, X25519 | TLS 1.2, ECDHE ChaCha20, X25519 | Handshake signature | Certificate chain |
|---|---|---|---|---|
| imap.gmail.com, smtp.gmail.com | yes | yes (the server switches to an RSA certificate) | TLS 1.3: ecdsa_secp256r1_sha256; TLS 1.2: rsa_pss_rsae_sha256 | leaf (P-256) ← WE2 (P-256 key, ECDSA with SHA-256) ← GTS Root R4 (P-384 key, ECDSA with SHA-384) |
| imap.hostinger.com, smtp.hostinger.com | yes | yes | rsa_pss_rsae_sha256 (RSA 2048) | leaf (RSA 2048) ← Sectigo DV R36 (RSA 3072, PKCS#1 with SHA-256) ← Sectigo Root R46 (RSA 4096, PKCS#1 with SHA-384), also cross-signed by USERTrust RSA (4096, SHA-384) |

In the chain column, each issuer is followed by its own key and the algorithm it signs the certificate on its left with.

A sample of other providers, for the `custom` preset:

| Algorithms dail implements | Servers that work |
|---|---|
| X25519 + ChaCha20-Poly1305 (tier 1, this plan) | Gmail, Hostinger, Yahoo, iCloud, Fastmail, Zoho, GMX, AOL, Yandex, IONOS, one.com, Migadu (ECDSA leaf), web.de, mailbox.org, Purelymail; mail.ru (TLS 1.2 only) |
| + AES-GCM (tier 2) | + Namecheap Private Email (mail.privateemail.com) |
| + P-256/P-384 key exchange (tier 3) | + Microsoft 365 / Outlook (outlook.office365.com, smtp.office365.com: AES-GCM with P-256 or P-384, no X25519); T-Online (secureimap.t-online.de: AES-GCM with P-384 only) |

RFC 8446 §9.1 makes AES-128-GCM and P-256 mandatory to implement. Tier 1 skips both on purpose, which is why the last
three providers fail. Microsoft's IMAP needs OAuth2, which dail doesn't support yet (ROADMAP), so tiers 2 and 3 belong
with that item (§11).

## 3. Scope

In scope (tier 1):

- TLS 1.3 and TLS 1.2.
- Cipher suites: `TLS13_CHACHA20_POLY1305_SHA256` (0x1303), `TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256` (0xCCA9),
  `TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256` (0xCCA8).
- Key exchange: X25519 only.
- Signature verification, for certificates and the handshake: the ring provider's table without Ed25519.
  - ECDSA with P-256 and P-384 keys, each with SHA-256 and SHA-384.
  - RSA PKCS#1 v1.5 with SHA-256/384/512, 2048–8192-bit keys, with and without NULL parameters.
  - RSA-PSS with SHA-256/384/512 (rsaEncryption keys; salt length = hash length).
- Hashes: SHA-256 for the suites (transcript, HKDF, the TLS 1.2 PRF). SHA-384 and SHA-512 only inside signature
  verification. All from the `sha2` crate, which app-io already uses.
- HMAC-SHA256. HKDF and the TLS 1.2 PRF come from rustls (`HkdfUsingHmac`, `PrfUsingHmac`).
- Random numbers from the OS through `getrandom` (already in the tree via `uuid`).

Out of scope: AES-GCM and P-256/P-384 key exchange (§11), Ed25519 and P-521 certificates, client certificates (no
signing at all), the server side, QUIC, kTLS key extraction, FIPS.

What the user sees: a server that offers none of these algorithms fails the handshake. dail then shows "the server
supports none of the TLS ciphers dail implements (TLS 1.2/1.3 with ChaCha20-Poly1305 and X25519)" instead of a bare
alert (§6).

## 4. Design

### 4.1 Crate and dependencies

`crates/app-tls`: the rustls crypto provider for dail's TLS client. No I/O except the OS random source; no tokio.

- Dependency edges: `app-tls ← (nothing internal)`; `app-sync ← app-core, app-io, app-tls`.
- `app-tls` may depend only on `rustls` (no provider features), `sha2`, `getrandom` and `zeroize`. Dev-dependencies:
  `serde`, `serde_json` and `proptest`, plus optional RustCrypto crates as test oracles (§7.2).
- Safe Rust only: the workspace's `unsafe_code = "forbid"` applies unchanged.
- `[profile.dev.package]` gets `app-tls = { opt-level = 2 }`, so debug builds and tests run the crypto at full speed.
- Size: about 1,900 lines of code and 900 lines of tests.

### 4.2 Modules

| Module | Contents | Handles secrets | Lines (est.) |
|---|---|---|---|
| `ct.rs` | constant-time helpers: masks, select, swap, equality | yes | 40 |
| `chacha20.rs` | ChaCha20 block function and keystream (RFC 8439 §2.1–2.4) | yes | 80 |
| `poly1305.rs` | Poly1305 (RFC 8439 §2.5) | yes | 130 |
| `aead.rs` | ChaCha20-Poly1305 (RFC 8439 §2.8) and the rustls record encrypters for TLS 1.3 and 1.2 | yes | 200 |
| `x25519.rs` | arithmetic mod 2^255 − 19, the Montgomery ladder (RFC 7748), the rustls key exchange | yes | 250 |
| `hash.rs` | the rustls `Hash`/`Context` over `sha2`; HMAC-SHA256 (RFC 2104) | yes (HMAC keys) | 120 |
| `der.rs` | strict DER reader: SEQUENCE and INTEGER | no | 80 |
| `bignum.rs` | variable-time unsigned integers: compare, add, subtract, Montgomery multiply and exponentiate | no | 220 |
| `rsa.rs` | PKCS#1 v1.5 and PSS verification (RFC 8017) | no | 180 |
| `ec.rs` | P-256/P-384 point decoding, Jacobian arithmetic, ECDSA verification | no | 320 |
| `verify.rs` | the `SignatureVerificationAlgorithm` table and the signature-scheme mapping | no | 150 |
| `suites.rs`, `rng.rs`, `lib.rs` | cipher suites, `SecureRandom`, `KeyProvider`, `provider()` | — | 150 |

### 4.3 Public API

```rust
/// The crypto provider for dail's TLS connections (ADR 0003).
pub fn provider() -> rustls::crypto::CryptoProvider;
```

Nothing else is public. Tests that need the primitives are unit tests inside the crate.

### 4.4 Mapping onto rustls 0.23

Use rustls's own ring provider as the template: `rustls-0.23.*/src/crypto/ring/{mod,tls13,tls12,kx,hash,hmac}.rs` in
`~/.cargo/registry/src/*/`. The names below were checked against rustls 0.23.46.

| rustls item | In app-tls |
|---|---|
| `CryptoProvider { cipher_suites, kx_groups, signature_verification_algorithms, secure_random, key_provider }` | `provider()` |
| `Tls13CipherSuite { common: CipherSuiteCommon { suite, hash_provider, confidentiality_limit: u64::MAX }, hkdf_provider: &HkdfUsingHmac(&HMAC_SHA256), aead_alg, quic: None }` | `TLS13_CHACHA20_POLY1305_SHA256` |
| `Tls12CipherSuite { common, prf_provider: &PrfUsingHmac(&HMAC_SHA256), kx: KeyExchangeAlgorithm::ECDHE, sign, aead_alg }` | the two ECDHE suites. `sign`: `ECDSA_NISTP384_SHA384`, `ECDSA_NISTP256_SHA256` for ECDSA; `RSA_PSS_SHA512` … `RSA_PKCS1_SHA256` for RSA |
| `Tls13AeadAlgorithm`: `encrypter`, `decrypter`, `key_len` = 32, `extract_keys` → `Err(UnsupportedOperationError)` | ChaCha20-Poly1305 |
| `Tls12AeadAlgorithm`: `key_block_shape` = `KeyBlockShape { enc_key_len: 32, fixed_iv_len: 12, explicit_nonce_len: 0 }` (RFC 7905) | ChaCha20-Poly1305 |
| `MessageEncrypter` / `MessageDecrypter` | nonce `Nonce::new(&iv, seq)`; AAD `make_tls13_aad(len)` or `make_tls12_aad(seq, typ, version, len)`. TLS 1.3 appends the content type before sealing and returns `into_tls13_unpadded_message()` after opening |
| `crypto::hash::{Hash, Context}` | SHA-256 |
| `crypto::hmac::{Hmac, Key}` | HMAC-SHA256 |
| `SupportedKxGroup` / `ActiveKeyExchange` | X25519 (`NamedGroup::X25519`) |
| `pki_types::SignatureVerificationAlgorithm` and `WebPkiSupportedAlgorithms { all, mapping }` | `verify.rs`. Identifiers from `rustls::pki_types::alg_id`. The RSA "absent parameters" identifiers are `AlgorithmIdentifier::from_slice` of the bare OID, e.g. `06 09 2a 86 48 86 f7 0d 01 01 0b` for SHA-256 |
| `SecureRandom` | `getrandom::fill`; an error becomes `GetRandomFailed` |
| `KeyProvider` | refuses every key: dail never presents a certificate |

The `mapping` mirrors ring's, minus Ed25519. For TLS 1.2 the curve isn't fixed by the scheme, so
`ECDSA_NISTP384_SHA384` maps to `[P384_SHA384, P256_SHA384]` and `ECDSA_NISTP256_SHA256` to
`[P256_SHA256, P384_SHA256]`. Each `RSA_PSS_SHA*` and `RSA_PKCS1_SHA*` scheme maps to its single algorithm.

### 4.5 Rules for the crypto code

1. **Secret data** (keys, X25519 scalars and field values, the ChaCha20 state, the Poly1305 accumulator, tags):
   - Nothing branches on it, indexes with it, divides by it or returns early because of it.
   - Masks come from `0u64.wrapping_sub(bit)` passed through `core::hint::black_box`.
   - Tags are compared by OR-accumulating the XOR of every byte.
   - Check the optimized code of `ct::swap`, `ct::select` and the Poly1305 final reduction for branches
     (`objdump -d`).
2. **Public data** (keys, signatures and messages in verification) may take variable time, but the checks must be
   exact (§5.6, §5.7).
3. **No panic on any input.** Check every length before indexing; prefer fixed-size arrays and `get`. Every parser and
   verifier gets a property test that feeds it random bytes and expects an error, never a panic.
4. **Zeroize secrets on drop** (`zeroize`): the ChaCha20 keys in the encrypters, the X25519 private key, and the HMAC
   key blocks.
5. **Cite the spec section** in the doc comment of every function.

## 5. Components

### 5.1 ChaCha20-Poly1305 (RFC 8439)

- ChaCha20 (§2.3, §2.4): a 32-bit block counter that starts at 1 for encryption. Block 0 gives the Poly1305 key
  (§2.6).
- Poly1305 (§2.5):
  - Clamp `r`.
  - Keep a 130-bit accumulator (five 26-bit limbs, or three 44-bit limbs with `u128` products).
  - Do the final reduction mod 2^130 − 5 and the addition of `s` mod 2^128 with masks, not branches.
- AEAD (§2.8):
  - The MAC covers `AAD ‖ pad16 ‖ ciphertext ‖ pad16 ‖ le64(AAD length) ‖ le64(ciphertext length)`.
  - On decryption, compute and check the tag over the ciphertext first, and decrypt only if it matches.
- Tests:
  - RFC 8439 §2.1.1, §2.2.1, §2.3.2, §2.4.2, §2.5.2, §2.6.2, §2.8.2 and Appendix A.
  - Wycheproof `chacha20_poly1305_test.json`.

### 5.2 X25519 (RFC 7748)

- Field elements in five 51-bit limbs (`u64`, products in `u128`), as in curve25519-donna-c64. Invert by Fermat
  (exponent p − 2).
- §5:
  - Clamp the scalar.
  - Mask the top bit of `u`, and accept non-canonical `u` by reducing it.
  - Run 255 ladder steps with a constant-time swap, whatever the scalar.
  - Encode the result canonically.
- Reject an all-zero shared secret (RFC 8446 §7.4.2) with `Err(PeerMisbehaved::InvalidKeyShare.into())`. Also reject
  a peer key that isn't exactly 32 bytes.
- The private key is 32 bytes from `getrandom`.
- Tests:
  - RFC 7748 §5.2: both vectors, plus the iterated vectors for 1 and 1,000 rounds. The 1,000,000 round is
    `#[ignore]`.
  - RFC 7748 §6.1.
  - Wycheproof `x25519_test.json` (low-order and non-canonical points).

### 5.3 SHA-2, HMAC, HKDF and the TLS 1.2 PRF

- The rustls `Hash` is SHA-256 (`output_len` 32). `Context::fork` clones the `sha2` state.
- HMAC (RFC 2104) uses a block size of 64 for SHA-256. Keys longer than a block are hashed first.
- HKDF and the PRF are rustls's `HkdfUsingHmac` and `PrfUsingHmac` over this HMAC.
- Tests:
  - RFC 4231 HMAC-SHA256 vectors and Wycheproof `hmac_sha256_test.json`.
  - RFC 5869 A.1–A.3 through `HkdfUsingHmac`.

### 5.4 DER reader

- Read only what the verifiers need: SEQUENCE and INTEGER, enough for `RSAPublicKey` and ECDSA signatures.
- Be strict:
  - Definite, minimally encoded lengths only.
  - INTEGERs minimal and non-negative.
  - No trailing bytes.

### 5.5 Big integers (verification only)

- Little-endian `u64` limbs. Operations:
  - conversion from and to big-endian bytes, compare, add, subtract;
  - Montgomery multiplication (CIOS) and exponentiation, with R² mod n.
- Variable time is fine: every input is public.
- Used by RSA (moduli up to 8192 bits) and for the P-256/P-384 field and scalar arithmetic in `ec.rs`.

### 5.6 RSA verification (RFC 8017)

- The public key is the DER `RSAPublicKey { modulus, publicExponent }`. These are the `subjectPublicKey` bytes that
  webpki passes.
- Reject:
  - a modulus outside 2048–8192 bits;
  - an even exponent, or one outside 3 ≤ e < 2^33 (ring's bounds);
  - a signature whose length differs from the modulus length in bytes;
  - s ≥ n.
- PKCS#1 v1.5 (§8.2.2, EMSA-PKCS1-v1_5 §9.2): build the expected encoding,
  `00 01 FF…FF 00 ‖ DigestInfo ‖ hash`, and compare it as a whole with sᵉ mod n. Never parse the padding: that is how
  Bleichenbacher-style forgeries get in.
- PSS (§8.1.2, EMSA-PSS-VERIFY §9.1.2, MGF1 in Appendix B.2.1):
  - The salt length must equal the hash length (RFC 8446 §4.2.3).
  - Check the trailing `0xbc`, the zero top bits, the zero padding and the `0x01` separator.
- Tests: Wycheproof
  - `rsa_signature_{2048_sha256,2048_sha384,2048_sha512,3072_sha256,3072_sha384,4096_sha384,4096_sha512}_test.json`
  - `rsa_pss_{2048_sha256_mgf1_32,2048_sha384_mgf1_48,4096_sha512_mgf1_64}_test.json`

### 5.7 ECDSA verification (SEC 1 v2 §4.1.4; curves from SEC 2 v2 §2.4.2 and §2.5.1)

- Public key: uncompressed points only (`04 ‖ X ‖ Y`, 65 or 97 bytes), with X, Y < p and the point on the curve.
- Signature: DER `SEQUENCE { r INTEGER, s INTEGER }`, read strictly (§5.4), with 1 ≤ r, s ≤ n − 1.
- `e` is the leftmost bits of the hash, truncated to the bit length of n. P-256 with SHA-384 takes 256 bits.
- Compute:
  - w = s⁻¹ mod n (Fermat), u₁ = e·w, u₂ = r·w;
  - R = u₁·G + u₂·Q in Jacobian coordinates with a = −3 (Explicit-Formulas Database "dbl-2001-b" and
    "add-2007-bl"), with both scalars interleaved (Shamir's trick).
- In the addition, handle P = Q (double instead) and P = −Q (the result is infinity) explicitly.
- Reject R = ∞. Accept if and only if R.x mod n = r.
- Tests: Wycheproof `ecdsa_secp256r1_sha256_test.json`, `ecdsa_secp384r1_sha256_test.json` and
  `ecdsa_secp384r1_sha384_test.json`. These use DER signatures; skip the `_p1363` files.

### 5.8 Provider

- `provider()` returns:
  - the three suites, TLS 1.3 first;
  - `[X25519]`;
  - the verification table;
  - the `getrandom` source;
  - the refusing key provider.
- A pinning test lists the suites, groups and verification algorithms exactly, the way `app-core/tests/it/registry.rs`
  pins the agent table, so any change to the set is deliberate.

## 6. Integration

1. **Workspace `Cargo.toml`:**
   - Add `crates/app-tls` to `members` and `default-members`.
   - `[workspace.dependencies]`: add `app-tls = { path = "crates/app-tls" }`, `getrandom = "0.4"` and `zeroize = "1"`.
     Both versions are already in the lock file, via `uuid` and `rustls`.
   - `rustls = { version = "0.23", default-features = false, features = ["std", "tls12", "logging"] }`.
   - `tokio-rustls = { version = "0.26", default-features = false, features = ["tls12", "logging"] }`.
   - lettre: replace `"tokio1-rustls-tls"` with `"tokio1-rustls", "rustls-no-provider", "webpki-roots"`.
   - `[profile.dev.package]`: `app-tls = { opt-level = 2 }`.

   On 2026-10-18 `cargo tree` confirmed that with these features:
   - `ring` disappears on every target;
   - `rustls-webpki` keeps only `alloc` and `std`;
   - `cc` remains only under `wayland-backend`, whose C log shim is off, and under two Haiku/Android crates.
2. **`crates/app-sync/src/tls.rs`:**
   - `client_config_with_roots` uses `ClientConfig::builder_with_provider(Arc::new(app_tls::provider()))`.
   - A new `smtp_parameters(host)`:
     - installs `app_tls::provider()` once as rustls's process-wide default. lettre's `rustls-no-provider` mode takes
       its provider from there and panics without one; `install_default` returns `Err` when one is already installed,
       which is fine;
     - builds `TlsParameters` with `CertificateStore::WebpkiRoots`.

     `smtp.rs` (`NetSmtp::send`) and `verify.rs` (`smtp_auth`) call it instead of building the parameters themselves.
   - The hint for incompatible servers (§3), in one place:
     - IMAP: take the `rustls::Error` out of tokio-rustls's `io::Error` (`get_ref()` and `downcast_ref`).
     - SMTP: check `lettre::transport::smtp::Error::is_tls()`.
     - Add the hint for `AlertReceived(HandshakeFailure | ProtocolVersion | InsufficientSecurity)` and
       `PeerIncompatible(_)`.
   - Update the module doc: rustls with app-tls (ADR 0003).
3. **`deny.toml`:**
   - Ban `ring`, with the reason that app-tls replaces it (ADR 0003).
   - Drop `ring` from the `cc` wrappers and update the comment: no crate in the build compiles C any more.
4. **`crates/app-sync/tests/it/live_it.rs`:** add `tls_reaches_reference_providers`.
   - It is `#[ignore]` and needs `DAIL_IT_TLS=1` and network, but no credentials.
   - IMAP 993 and SMTP 465 at Gmail and Hostinger: read the greeting.
   - SMTP 587: STARTTLS through lettre (`AsyncSmtpConnection`, `starttls`, `quit`), no AUTH.
   - Assert TLS 1.3, `TLS13_CHACHA20_POLY1305_SHA256` and X25519 wherever the connection exposes them.

## 7. Tests

### 7.1 Test vectors

- **Wycheproof** files come from <https://github.com/C2SP/wycheproof>, directory `testvectors_v1/` (Apache-2.0).
  - Keep them unmodified in `crates/app-tls/testdata/wycheproof/`, about 3.8 MB, with a README that names the upstream
    commit.
  - Files: the ones named in §5.1, §5.2, §5.3, §5.6 and §5.7.
- **Policy:**
  - every `valid` case must verify or decrypt;
  - every `invalid` case must fail;
  - `acceptable` cases are rejected (strict). The test lists their flags.
- **RFC vectors** live inline in the unit tests.

### 7.2 Cross-checks (recommended)

- Property tests against RustCrypto crates, as dev-dependencies only. They are pure Rust and never linked into dail:
  - `x25519-dalek` and `chacha20poly1305`;
  - `p256` and `p384` with the `ecdsa` feature: sign random messages with them, then compare verification of the valid
    and of mutated signatures.
- Run `cargo deny check` after adding them.
- RSA is checked with Wycheproof only: the `rsa` crate carries RUSTSEC-2023-0071.

### 7.3 Robustness

Random bytes go into the DER reader, both verifiers and AEAD decryption. The result must always be an error, never a
panic.

### 7.4 Handshakes, in memory

- Connect a rustls `ClientConnection` and `ServerConnection` by passing bytes with `write_tls`, `read_tls` and
  `process_new_packets` in a loop. No sockets, no tokio.
- Both sides use `app_tls::provider()`. The server uses a test-only `KeyProvider` (`#[cfg(test)]`, built on `bignum`,
  `ec` and `der`) that reads the PKCS#8 test keys:
  - RSA signs PKCS#1 v1.5 and PSS (with a fixed salt);
  - ECDSA P-256 signs with a fixed nonce.

  None of this exists outside tests.
- Fixtures in `crates/app-tls/testdata/certs/` (Appendix B):
  - an ECDSA chain: P-384 root, P-256 server certificate signed with SHA-384, like Gmail;
  - an RSA chain: 3072-bit root, 2048-bit server certificate signed with SHA-256, like Hostinger;
  - both for `mail.dail.test`, valid until 2126.
- Cases that must succeed:
  - TLS 1.3 with each chain;
  - TLS 1.2 with each ECDHE suite, including RSA with both PKCS#1 and PSS;
  - a 1 MiB payload in each direction.

  Assert the negotiated suite and group in each.
- Cases that must fail:
  - unknown issuer;
  - wrong server name;
  - expired certificate (`ClientConfig::builder_with_details` with a `TimeProvider` past `notAfter`);
  - one flipped ciphertext bit;
  - an X25519 key share that gives an all-zero secret (`PeerMisbehaved::InvalidKeyShare`).

### 7.5 app-sync

- The incompatible-server hint: a unit test over constructed errors.
- `smtp_parameters` installs the process-wide default provider.

### 7.6 Commands for the final check

- `cargo xtask verify`: all six gates.
- `cargo tree -i ring --target all` prints nothing.
- With `rustup target add x86_64-pc-windows-msvc`, run `cargo check --workspace --all-targets --target
  x86_64-pc-windows-msvc`, and the same with `clippy -- -D warnings`. This is the first time Windows code can be
  checked on Linux; report the result.
- `DAIL_IT_TLS=1 cargo test -p app-sync --test it live_it::tls -- --ignored`. With credentials, also the login tests
  (`DAIL_IT_{GMAIL,HOSTINGER}_{EMAIL,PASSWORD}`).

## 8. Steps

Each step is one commit, green under `cargo xtask verify`, with a message of the form `<crate>: <imperative summary>`.

- [x] 1. `app-tls: add the crate with ChaCha20-Poly1305`
  - ADR 0003, from §1–§3 of this plan.
  - The crate: `Cargo.toml` with the workspace fields and `[lints] workspace = true`; `ct`, `chacha20`, `poly1305`,
    and the AEAD core of `aead`.
  - RFC and Wycheproof tests.
  - The workspace members.
  - The topology lines in AGENTS.md §2 and ARCHITECTURE §3.
  - An unchecked ROADMAP item pointing to this plan.
- [x] 2. `app-tls: add X25519`: `x25519` with its vectors, and the cross-check if §7.2 is adopted.
- [x] 3. `app-tls: verify RSA signatures`: `der`, `bignum`, `rsa` and the Wycheproof RSA tests.
- [x] 4. `app-tls: verify ECDSA signatures`: `ec`, the Wycheproof ECDSA tests and the cross-check.
- [x] 5. `app-tls: assemble the rustls provider`
  - `hash`, `rng`, `verify`, `suites`, `provider()`;
  - the pinning test and the in-memory handshake tests.
- [x] 6. `app-sync: use app-tls instead of ring`
  - the changes in §6, the hint and the live test;
  - the docs in §9, and the ROADMAP item ticked;
  - `cargo tree -i ring --target all` prints nothing.
- [x] 7. The final checks in §7.6, including the Windows check and the live test, with their results in the report.

### 8.1 Results of the final checks (2026-10-18)

- `cargo xtask verify`: all six gates pass.
- `cargo tree -i ring --target all` prints nothing. `cc` remains only under `wayland-backend`, `iana-time-zone-haiku`
  and `android-activity`, which compile nothing with it here.
- `cargo check` and `cargo clippy -- -D warnings` with `--workspace --all-targets --target x86_64-pc-windows-msvc`
  pass on Linux.
- `DAIL_IT_TLS=1 cargo test -p app-sync --test it live_it::tls -- --ignored` passes: TLS 1.3,
  `TLS13_CHACHA20_POLY1305_SHA256` and X25519 with imap.gmail.com and imap.hostinger.com on 993 and smtp.gmail.com and
  smtp.hostinger.com on 465; STARTTLS on 587 at both. The login tests did not run (no `DAIL_IT_*` credentials).
- The RFC 7748 iterated vector for 1,000,000 rounds passes (about 100 s, `#[ignore]`).
- §4.5 assembly check (release build, `--emit=asm`): every conditional jump in `x25519`, `Poly1305::update` and the
  inlined `finish`/`ct` helpers in `aead::tag` tests a loop counter or a public length; the masked selects compile
  without branches.
- Deviations from this plan: X25519 accepts Wycheproof's `acceptable` twist and non-canonical cases (§5.2 requires it;
  only all-zero secrets are refused). The SMTP hint walks the error's source chain instead of checking `is_tls()`:
  lettre reports handshake failures as connection errors.

## 9. Docs to update (within the steps above)

- **ADR 0003** (new).
- **ARCHITECTURE.md:**
  - §3 topology.
  - §5.3, the TLS paragraph: rustls with app-tls, the algorithms, the provider table of §2, the hint.
  - §9, the MITM row.
  - §11, the testing table: an app-tls row (vectors, cross-checks, handshakes).
  - §12, key dependencies: a TLS row.
- **AGENTS.md** (untracked: the edit stays local, so say so in the report):
  - §2 topology and dependency rules: `app-tls ← (nothing internal)` with its allowed dependencies (§4.1), and
    `app-sync ← app-core, app-io, app-tls`.
  - §5 "Pure-Rust deps": no exception left; `ring` is banned.
- **ROADMAP.md:** Wave 2 gets a **Build & dependencies** group with this item. The tier 2/3 work goes under "Accounts &
  protocols", next to the OAuth2 item for Microsoft 365.
- **`deny.toml`:** the comments.

## 10. Risks and review checklist

| Risk | Effect | Mitigation |
|---|---|---|
| Verification accepts a forged signature | a man in the middle reads the password | Wycheproof; cross-checks; whole-encoding PKCS#1 comparison; strict DER; range and on-curve checks; the checklist below |
| Timing leak in secret-handling code | in theory, key recovery; in practice little, since every key is used once | the rules in §4.5; the assembly check |
| Bug in the AEAD or X25519 | handshakes or records fail (closed), or weaker security | RFC and Wycheproof vectors; cross-checks |
| No external audit | nobody outside the project has read the code | keep the scope small; get an outside review before a public release |
| rustls API changes (0.24 is in development) | the glue breaks on upgrade | the glue stays in `suites`, `aead`, `verify` and `hash`; upgrade together with the tests |
| Servers that need AES-GCM or NIST key exchange | dail can't connect to them | the hint (§6); tiers 2 and 3 (§11) |

Review checklist before step 6 is merged:

Self-review on 2026-10-18 (an outside review is still needed before a public release, §10):

- [x] `ct` helpers: no branches in the optimized code.
- [x] ChaCha20-Poly1305: the tag is checked before any plaintext is released, in constant time; the block counter
  can't wrap within a record.
- [x] X25519: scalar clamped; top bit of `u` masked; 255 ladder steps whatever the scalar; all-zero secret rejected.
- [x] RSA:
  - exact signature length, s < n;
  - modulus 2048–8192 bits; e odd and in range;
  - PKCS#1 compares the whole encoding; PSS salt length = hash length.
- [x] ECDSA:
  - r, s ∈ [1, n − 1]; strict DER;
  - point below p and on the curve; infinity rejected;
  - P = ±Q handled; hash truncated to the bit length of n.
- [x] DER: rejects indefinite and non-minimal lengths, negative or non-minimal INTEGERs, and trailing bytes.
- [x] No `unsafe`; no panic on random input; secrets zeroized; the pinning test matches §3.

## 11. Later: tiers 2 and 3

- **AES-128/256-GCM** (tier 2):
  - AES bitsliced in constant time (the design of BearSSL's `aes_ct64`), and GHASH by constant-time integer
    multiplication (BearSSL's `ghash_ctmul64`). About 650 lines, slower than AES-NI but fast enough for mail.
  - Suites: `TLS13_AES_128_GCM_SHA256`, `TLS13_AES_256_GCM_SHA384` (which also needs HMAC/HKDF over SHA-384), and the
    TLS 1.2 ECDHE AES-GCM suites.
- **P-256/P-384 key exchange** (tier 3): constant-time scalar multiplication with complete addition formulas
  (Renes–Costello–Batina 2016) and constant-time table lookups, on constant-time field arithmetic. About 300 lines.
- **When:** with the Microsoft 365 / OAuth2 item, or when a user's `custom` server needs it.

## Appendix A: server probe

Run with bash: zsh doesn't split `$starttls` into two arguments. It does one handshake per algorithm set, sends nothing
after the handshake, and needs only the `openssl` command-line tool.

```bash
#!/usr/bin/env bash
# Usage: bash probe.sh HOST PORT [smtp|imap]   (the third argument turns on STARTTLS for that protocol)
host=$1 port=$2 starttls=${3:+-starttls $3}
try() {
    local label=$1 out; shift
    out=$(timeout 20 openssl s_client -connect "$host:$port" -servername "$host" $starttls "$@" </dev/null 2>&1)
    if grep -qE '^New, TLSv1\.[23]' <<<"$out"; then
        printf '%-22s ok    %s | %s | %s\n' "$label" \
            "$(grep -m1 -oE 'TLSv1\.[23], Cipher is [A-Z0-9_-]+' <<<"$out")" \
            "$(grep -m1 -oE '(Peer Temp Key|Negotiated TLS1.3 group): [^,]+' <<<"$out")" \
            "$(grep -m1 -oE 'Peer signature type: .+' <<<"$out")"
    else
        printf '%-22s FAIL  %s\n' "$label" "$(grep -m1 -oE 'alert [a-z ]+' <<<"$out")"
    fi
}
echo "$host:$port"
try "tls1.3 chacha x25519" -tls1_3 -ciphersuites TLS_CHACHA20_POLY1305_SHA256 -groups X25519
try "tls1.2 chacha x25519" -tls1_2 -cipher ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305 -groups X25519
try "tls1.3 aes128 p256" -tls1_3 -ciphersuites TLS_AES_128_GCM_SHA256 -groups P-256
try "server's choice"
# The chain the server sends: each certificate's key and the algorithm it is signed with.
timeout 20 openssl s_client -connect "$host:$port" -servername "$host" $starttls -showcerts </dev/null 2>/dev/null |
    grep -E '^ *[0-9]+ s:|^ +a:PKEY'
```

For example: `bash probe.sh imap.gmail.com 993`, then `bash probe.sh smtp.hostinger.com 587 smtp`.

## Appendix B: test certificates

Throwaway test keys, valid until 2126. Commit the root certificates, the server certificates and the server keys. Keep
the root keys in `/tmp`, and add a README saying the keys are for tests only. Run from the repository root:

```bash
d=crates/app-tls/testdata/certs; mkdir -p "$d"
printf '%s\n' 'subjectAltName=DNS:mail.dail.test' 'basicConstraints=critical,CA:FALSE' \
    'keyUsage=critical,digitalSignature,keyEncipherment' 'extendedKeyUsage=serverAuth' > /tmp/server.ext
ca='-addext basicConstraints=critical,CA:TRUE -addext keyUsage=critical,keyCertSign,cRLSign'
# ECDSA: a P-384 root signs a P-256 server certificate with SHA-384, as in Gmail's chain.
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:secp384r1 -nodes -days 36500 $ca \
    -subj '/CN=dail test root (ECDSA)' -keyout /tmp/root-ecdsa.key -out "$d/root-ecdsa.pem"
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -subj '/CN=mail.dail.test' \
    -keyout "$d/server-ecdsa.key" -out /tmp/server-ecdsa.csr
openssl x509 -req -in /tmp/server-ecdsa.csr -CA "$d/root-ecdsa.pem" -CAkey /tmp/root-ecdsa.key -set_serial 1 \
    -days 36500 -sha384 -extfile /tmp/server.ext -out "$d/server-ecdsa.pem"
# RSA: a 3072-bit root signs a 2048-bit server certificate with SHA-256, as in Hostinger's chain.
openssl req -x509 -newkey rsa:3072 -nodes -days 36500 $ca \
    -subj '/CN=dail test root (RSA)' -keyout /tmp/root-rsa.key -out "$d/root-rsa.pem"
openssl req -newkey rsa:2048 -nodes -subj '/CN=mail.dail.test' -keyout "$d/server-rsa.key" -out /tmp/server-rsa.csr
openssl x509 -req -in /tmp/server-rsa.csr -CA "$d/root-rsa.pem" -CAkey /tmp/root-rsa.key -set_serial 2 \
    -days 36500 -sha256 -extfile /tmp/server.ext -out "$d/server-rsa.pem"
openssl verify -CAfile "$d/root-ecdsa.pem" "$d/server-ecdsa.pem"
openssl verify -CAfile "$d/root-rsa.pem" "$d/server-rsa.pem"
```

`openssl req` writes the keys as PKCS#8 (`BEGIN PRIVATE KEY`), which the test-only key provider reads (§7.4).
