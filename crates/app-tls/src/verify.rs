//! The signature verification algorithms for certificates (`rustls-webpki`) and the handshake (rustls): ring's table
//! without Ed25519 (`docs/plan-tls.md` §3, §4.4).

use rustls::SignatureScheme;
use rustls::crypto::WebPkiSupportedAlgorithms;
use rustls::pki_types::{
    AlgorithmIdentifier, InvalidSignature, SignatureVerificationAlgorithm, alg_id,
};

use crate::ec::{self, CurveId};
use crate::hash::SigHash;
use crate::rsa;

/// How a signature is checked.
#[derive(Debug, Clone, Copy)]
enum Scheme {
    Ecdsa(CurveId),
    RsaPkcs1,
    RsaPss,
}

/// One verification algorithm: the identifiers webpki matches it by, and how to check a signature.
#[derive(Debug)]
struct Algorithm {
    public_key_alg: AlgorithmIdentifier,
    signature_alg: AlgorithmIdentifier,
    scheme: Scheme,
    hash: SigHash,
}

impl SignatureVerificationAlgorithm for Algorithm {
    /// `public_key` is the `subjectPublicKey` value: an uncompressed point for ECDSA, a DER `RSAPublicKey` for RSA.
    fn verify_signature(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), InvalidSignature> {
        let verdict = match self.scheme {
            Scheme::Ecdsa(curve) => ec::verify(curve, self.hash, public_key, message, signature),
            Scheme::RsaPkcs1 => rsa::PublicKey::from_der(public_key)
                .and_then(|key| key.verify_pkcs1(self.hash, message, signature)),
            Scheme::RsaPss => rsa::PublicKey::from_der(public_key)
                .and_then(|key| key.verify_pss(self.hash, message, signature)),
        };
        verdict.map_err(|crate::Error| InvalidSignature)
    }

    fn public_key_alg_id(&self) -> AlgorithmIdentifier {
        self.public_key_alg
    }

    fn signature_alg_id(&self) -> AlgorithmIdentifier {
        self.signature_alg
    }
}

const fn ecdsa(curve: CurveId, hash: SigHash) -> Algorithm {
    let (public_key_alg, signature_alg) = match (curve, hash) {
        (CurveId::P256, SigHash::Sha256) => (alg_id::ECDSA_P256, alg_id::ECDSA_SHA256),
        (CurveId::P256, _) => (alg_id::ECDSA_P256, alg_id::ECDSA_SHA384),
        (CurveId::P384, SigHash::Sha256) => (alg_id::ECDSA_P384, alg_id::ECDSA_SHA256),
        (CurveId::P384, _) => (alg_id::ECDSA_P384, alg_id::ECDSA_SHA384),
    };
    Algorithm {
        public_key_alg,
        signature_alg,
        scheme: Scheme::Ecdsa(curve),
        hash,
    }
}

const fn rsa(scheme: Scheme, signature_alg: AlgorithmIdentifier, hash: SigHash) -> Algorithm {
    Algorithm {
        public_key_alg: alg_id::RSA_ENCRYPTION,
        signature_alg,
        scheme,
        hash,
    }
}

static ECDSA_P256_SHA256: Algorithm = ecdsa(CurveId::P256, SigHash::Sha256);
static ECDSA_P256_SHA384: Algorithm = ecdsa(CurveId::P256, SigHash::Sha384);
static ECDSA_P384_SHA256: Algorithm = ecdsa(CurveId::P384, SigHash::Sha256);
static ECDSA_P384_SHA384: Algorithm = ecdsa(CurveId::P384, SigHash::Sha384);

// RSA-PSS with an `rsaEncryption` key (webpki's "legacy key" variants), as ring's provider has them.
static RSA_PSS_SHA256: Algorithm = rsa(Scheme::RsaPss, alg_id::RSA_PSS_SHA256, SigHash::Sha256);
static RSA_PSS_SHA384: Algorithm = rsa(Scheme::RsaPss, alg_id::RSA_PSS_SHA384, SigHash::Sha384);
static RSA_PSS_SHA512: Algorithm = rsa(Scheme::RsaPss, alg_id::RSA_PSS_SHA512, SigHash::Sha512);

static RSA_PKCS1_SHA256: Algorithm =
    rsa(Scheme::RsaPkcs1, alg_id::RSA_PKCS1_SHA256, SigHash::Sha256);
static RSA_PKCS1_SHA384: Algorithm =
    rsa(Scheme::RsaPkcs1, alg_id::RSA_PKCS1_SHA384, SigHash::Sha384);
static RSA_PKCS1_SHA512: Algorithm =
    rsa(Scheme::RsaPkcs1, alg_id::RSA_PKCS1_SHA512, SigHash::Sha512);

// `sha*WithRSAEncryption` with the NULL parameters left out: RFC 4055 §5 says they MUST be NULL, but
// implementations MUST accept them absent. The identifier is then the bare OID (1.2.840.113549.1.1.11–13).
static RSA_PKCS1_SHA256_ABSENT_PARAMS: Algorithm = rsa(
    Scheme::RsaPkcs1,
    AlgorithmIdentifier::from_slice(&[
        0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b,
    ]),
    SigHash::Sha256,
);
static RSA_PKCS1_SHA384_ABSENT_PARAMS: Algorithm = rsa(
    Scheme::RsaPkcs1,
    AlgorithmIdentifier::from_slice(&[
        0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0c,
    ]),
    SigHash::Sha384,
);
static RSA_PKCS1_SHA512_ABSENT_PARAMS: Algorithm = rsa(
    Scheme::RsaPkcs1,
    AlgorithmIdentifier::from_slice(&[
        0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0d,
    ]),
    SigHash::Sha512,
);

/// Every algorithm, and the algorithms each TLS signature scheme may use. For ECDSA the TLS 1.2 schemes don't fix
/// the curve, so each maps to both curves (RFC 8422 §5.1.3); TLS 1.3 checks the curve itself (RFC 8446 §4.2.3).
pub(crate) static ALGORITHMS: WebPkiSupportedAlgorithms = WebPkiSupportedAlgorithms {
    all: &[
        &ECDSA_P256_SHA256,
        &ECDSA_P256_SHA384,
        &ECDSA_P384_SHA256,
        &ECDSA_P384_SHA384,
        &RSA_PSS_SHA256,
        &RSA_PSS_SHA384,
        &RSA_PSS_SHA512,
        &RSA_PKCS1_SHA256,
        &RSA_PKCS1_SHA384,
        &RSA_PKCS1_SHA512,
        &RSA_PKCS1_SHA256_ABSENT_PARAMS,
        &RSA_PKCS1_SHA384_ABSENT_PARAMS,
        &RSA_PKCS1_SHA512_ABSENT_PARAMS,
    ],
    mapping: &[
        (
            SignatureScheme::ECDSA_NISTP384_SHA384,
            &[&ECDSA_P384_SHA384, &ECDSA_P256_SHA384],
        ),
        (
            SignatureScheme::ECDSA_NISTP256_SHA256,
            &[&ECDSA_P256_SHA256, &ECDSA_P384_SHA256],
        ),
        (SignatureScheme::RSA_PSS_SHA512, &[&RSA_PSS_SHA512]),
        (SignatureScheme::RSA_PSS_SHA384, &[&RSA_PSS_SHA384]),
        (SignatureScheme::RSA_PSS_SHA256, &[&RSA_PSS_SHA256]),
        (SignatureScheme::RSA_PKCS1_SHA512, &[&RSA_PKCS1_SHA512]),
        (SignatureScheme::RSA_PKCS1_SHA384, &[&RSA_PKCS1_SHA384]),
        (SignatureScheme::RSA_PKCS1_SHA256, &[&RSA_PKCS1_SHA256]),
    ],
};
