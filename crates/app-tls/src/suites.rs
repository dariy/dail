//! The three cipher suites (`docs/plan-tls.md` §3): ChaCha20-Poly1305 with SHA-256, for TLS 1.3 and for TLS 1.2
//! with ECDHE and ECDSA or RSA certificates.

use rustls::crypto::tls12::PrfUsingHmac;
use rustls::crypto::tls13::HkdfUsingHmac;
use rustls::crypto::{CipherSuiteCommon, KeyExchangeAlgorithm};
use rustls::{
    CipherSuite, SignatureScheme, SupportedCipherSuite, Tls12CipherSuite, Tls13CipherSuite,
};

use crate::aead::ChaCha20Poly1305;
use crate::hash::{HmacSha256, Sha256};

/// ChaCha20-Poly1305 has no practical confidentiality limit (draft-irtf-cfrg-aead-limits §5.2.1).
const COMMON_LIMIT: u64 = u64::MAX;

/// `TLS_CHACHA20_POLY1305_SHA256` (0x1303, RFC 8446 §B.4).
pub(crate) static TLS13_CHACHA20_POLY1305_SHA256: SupportedCipherSuite =
    SupportedCipherSuite::Tls13(&Tls13CipherSuite {
        common: CipherSuiteCommon {
            suite: CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
            hash_provider: &Sha256,
            confidentiality_limit: COMMON_LIMIT,
        },
        hkdf_provider: &HkdfUsingHmac(&HmacSha256),
        aead_alg: &ChaCha20Poly1305,
        quic: None,
    });

/// `TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256` (0xCCA9, RFC 7905).
pub(crate) static TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256: SupportedCipherSuite =
    SupportedCipherSuite::Tls12(&Tls12CipherSuite {
        common: CipherSuiteCommon {
            suite: CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
            hash_provider: &Sha256,
            confidentiality_limit: COMMON_LIMIT,
        },
        kx: KeyExchangeAlgorithm::ECDHE,
        sign: &[
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP256_SHA256,
        ],
        aead_alg: &ChaCha20Poly1305,
        prf_provider: &PrfUsingHmac(&HmacSha256),
    });

/// `TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256` (0xCCA8, RFC 7905).
pub(crate) static TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256: SupportedCipherSuite =
    SupportedCipherSuite::Tls12(&Tls12CipherSuite {
        common: CipherSuiteCommon {
            suite: CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
            hash_provider: &Sha256,
            confidentiality_limit: COMMON_LIMIT,
        },
        kx: KeyExchangeAlgorithm::ECDHE,
        sign: &[
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA256,
        ],
        aead_alg: &ChaCha20Poly1305,
        prf_provider: &PrfUsingHmac(&HmacSha256),
    });
