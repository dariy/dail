//! The crypto provider for dail's TLS client (ADR 0003, `docs/plan-tls.md`).
//!
//! rustls keeps the protocol and `rustls-webpki` the certificate parsing; this crate supplies only the algorithms
//! dail's TLS client needs, in safe Rust: ChaCha20-Poly1305, X25519, SHA-256/HMAC, and RSA and ECDSA signature
//! verification. Code that handles secrets follows the constant-time rules of the plan (§4.5).

mod aead;
mod bignum;
mod chacha20;
mod ct;
mod der;
mod ec;
mod hash;
mod poly1305;
mod rng;
mod rsa;
mod suites;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod testutil;
mod verify;
mod x25519;

use rustls::crypto::CryptoProvider;

/// A failed cryptographic operation. It carries no detail on purpose: a caller learns only that the input was
/// refused, never which check refused it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Error;

/// The crypto provider for dail's TLS connections (ADR 0003): TLS 1.3 and TLS 1.2 with ChaCha20-Poly1305, X25519 key
/// exchange, and ECDSA (P-256/P-384) and RSA (2048–8192 bits, PKCS#1 v1.5 and PSS) signature verification. It loads
/// no private keys.
pub fn provider() -> CryptoProvider {
    CryptoProvider {
        cipher_suites: vec![
            suites::TLS13_CHACHA20_POLY1305_SHA256,
            suites::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
            suites::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
        ],
        kx_groups: vec![&x25519::X25519],
        signature_verification_algorithms: verify::ALGORITHMS,
        secure_random: &rng::OsRandom,
        key_provider: &rng::NoKeys,
    }
}
