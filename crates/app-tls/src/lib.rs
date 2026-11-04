//! The crypto provider for dail's TLS client (ADR 0003, amended by ADR 0014).
//!
//! rustls keeps the protocol and `rustls-webpki` the certificate parsing. The primitives come from audited or widely
//! used crates (ADR 0014 lists each and its audit): `chacha20poly1305`, `aes-gcm`, `x25519-dalek`, `p256`, `p384`,
//! `rsa` and `sha2`. This crate is the glue: cipher suites, record protection, key exchange groups and signature
//! verification as rustls asks for them, in safe Rust. It loads no private keys.

mod aead;
mod ec;
mod hash;
mod kx;
mod rng;
mod rsa;
mod suites;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod testutil;
mod verify;

use rustls::crypto::CryptoProvider;

/// A failed cryptographic operation. It carries no detail on purpose: a caller learns only that the input was
/// refused, never which check refused it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Error;

/// The crypto provider for dail's TLS connections (ADR 0003, 0014): TLS 1.3 and TLS 1.2 with ChaCha20-Poly1305 and
/// AES-128/256-GCM, X25519, P-256 and P-384 key exchange, and ECDSA (P-256/P-384) and RSA (2048–8192 bits, PKCS#1
/// v1.5 and PSS) signature verification. It loads no private keys.
pub fn provider() -> CryptoProvider {
    CryptoProvider {
        cipher_suites: suites::ALL.to_vec(),
        kx_groups: vec![&kx::X25519, &kx::SECP256R1, &kx::SECP384R1],
        signature_verification_algorithms: verify::ALGORITHMS,
        secure_random: &rng::OsRandom,
        key_provider: &rng::NoKeys,
    }
}
