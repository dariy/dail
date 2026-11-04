//! The cipher suites (`docs/plan-tls.md` §3, ADR 0014): ChaCha20-Poly1305 and AES-128/256-GCM, for TLS 1.3 and for
//! TLS 1.2 with ECDHE and ECDSA or RSA certificates. ChaCha20-Poly1305 comes first; AES-GCM is what servers without
//! it (Microsoft 365 among them) need.

use rustls::crypto::tls12::PrfUsingHmac;
use rustls::crypto::tls13::HkdfUsingHmac;
use rustls::crypto::{CipherSuiteCommon, KeyExchangeAlgorithm};
use rustls::{
    CipherSuite, SignatureScheme, SupportedCipherSuite, Tls12CipherSuite, Tls13CipherSuite,
};

use crate::aead::{AES_128_GCM, AES_256_GCM, Aead, CHACHA20_POLY1305};
use crate::hash::{HMAC_SHA256, HMAC_SHA384, Hash, SHA256, SHA384};

/// ChaCha20-Poly1305 has no practical confidentiality limit (draft-irtf-cfrg-aead-limits §5.2.1).
const CHACHA_LIMIT: u64 = u64::MAX;
/// AES-GCM: 2^24.5 full-size records keep the attacker's advantage below 2^-57 (RFC 8446 §5.5, rustls's value).
const GCM_LIMIT: u64 = 1 << 23;

const ECDSA_SCHEMES: &[SignatureScheme] = &[
    SignatureScheme::ECDSA_NISTP384_SHA384,
    SignatureScheme::ECDSA_NISTP256_SHA256,
];

const RSA_SCHEMES: &[SignatureScheme] = &[
    SignatureScheme::RSA_PSS_SHA512,
    SignatureScheme::RSA_PSS_SHA384,
    SignatureScheme::RSA_PSS_SHA256,
    SignatureScheme::RSA_PKCS1_SHA512,
    SignatureScheme::RSA_PKCS1_SHA384,
    SignatureScheme::RSA_PKCS1_SHA256,
];

const fn common(suite: CipherSuite, hash: &'static Hash, limit: u64) -> CipherSuiteCommon {
    CipherSuiteCommon {
        suite,
        hash_provider: hash,
        confidentiality_limit: limit,
    }
}

static HKDF_SHA256: HkdfUsingHmac<'static> = HkdfUsingHmac(&HMAC_SHA256);
static HKDF_SHA384: HkdfUsingHmac<'static> = HkdfUsingHmac(&HMAC_SHA384);
static PRF_SHA256: PrfUsingHmac<'static> = PrfUsingHmac(&HMAC_SHA256);
static PRF_SHA384: PrfUsingHmac<'static> = PrfUsingHmac(&HMAC_SHA384);

const fn tls13(
    suite: CipherSuite,
    hash: &'static Hash,
    hkdf: &'static HkdfUsingHmac<'static>,
    aead: &'static Aead,
    limit: u64,
) -> Tls13CipherSuite {
    Tls13CipherSuite {
        common: common(suite, hash, limit),
        hkdf_provider: hkdf,
        aead_alg: aead,
        quic: None,
    }
}

const fn tls12(
    suite: CipherSuite,
    sign: &'static [SignatureScheme],
    hash: &'static Hash,
    prf: &'static PrfUsingHmac<'static>,
    aead: &'static Aead,
    limit: u64,
) -> Tls12CipherSuite {
    Tls12CipherSuite {
        common: common(suite, hash, limit),
        kx: KeyExchangeAlgorithm::ECDHE,
        sign,
        aead_alg: aead,
        prf_provider: prf,
    }
}

static T13_CHACHA: Tls13CipherSuite = tls13(
    CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
    &SHA256,
    &HKDF_SHA256,
    &CHACHA20_POLY1305,
    CHACHA_LIMIT,
);
static T13_AES128: Tls13CipherSuite = tls13(
    CipherSuite::TLS13_AES_128_GCM_SHA256,
    &SHA256,
    &HKDF_SHA256,
    &AES_128_GCM,
    GCM_LIMIT,
);
static T13_AES256: Tls13CipherSuite = tls13(
    CipherSuite::TLS13_AES_256_GCM_SHA384,
    &SHA384,
    &HKDF_SHA384,
    &AES_256_GCM,
    GCM_LIMIT,
);

static T12_ECDSA_CHACHA: Tls12CipherSuite = tls12(
    CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
    ECDSA_SCHEMES,
    &SHA256,
    &PRF_SHA256,
    &CHACHA20_POLY1305,
    CHACHA_LIMIT,
);
static T12_RSA_CHACHA: Tls12CipherSuite = tls12(
    CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
    RSA_SCHEMES,
    &SHA256,
    &PRF_SHA256,
    &CHACHA20_POLY1305,
    CHACHA_LIMIT,
);
static T12_ECDSA_AES128: Tls12CipherSuite = tls12(
    CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
    ECDSA_SCHEMES,
    &SHA256,
    &PRF_SHA256,
    &AES_128_GCM,
    GCM_LIMIT,
);
static T12_ECDSA_AES256: Tls12CipherSuite = tls12(
    CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
    ECDSA_SCHEMES,
    &SHA384,
    &PRF_SHA384,
    &AES_256_GCM,
    GCM_LIMIT,
);
static T12_RSA_AES128: Tls12CipherSuite = tls12(
    CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
    RSA_SCHEMES,
    &SHA256,
    &PRF_SHA256,
    &AES_128_GCM,
    GCM_LIMIT,
);
static T12_RSA_AES256: Tls12CipherSuite = tls12(
    CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
    RSA_SCHEMES,
    &SHA384,
    &PRF_SHA384,
    &AES_256_GCM,
    GCM_LIMIT,
);

/// Every suite, in preference order.
pub(crate) static ALL: [SupportedCipherSuite; 9] = [
    SupportedCipherSuite::Tls13(&T13_CHACHA),
    SupportedCipherSuite::Tls13(&T13_AES256),
    SupportedCipherSuite::Tls13(&T13_AES128),
    SupportedCipherSuite::Tls12(&T12_ECDSA_CHACHA),
    SupportedCipherSuite::Tls12(&T12_RSA_CHACHA),
    SupportedCipherSuite::Tls12(&T12_ECDSA_AES256),
    SupportedCipherSuite::Tls12(&T12_ECDSA_AES128),
    SupportedCipherSuite::Tls12(&T12_RSA_AES256),
    SupportedCipherSuite::Tls12(&T12_RSA_AES128),
];
