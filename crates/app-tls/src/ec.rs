//! ECDSA signature verification on P-256 and P-384 with `p256` and `p384` (ADR 0014). Only the glue is dail's: the
//! curve and hash a TLS or X.509 algorithm names, the uncompressed key, the DER signature. Everything here is public
//! data (keys, signatures, hashes).

use p256::ecdsa::signature::hazmat::PrehashVerifier;

use crate::Error;
use crate::hash::SigHash;

/// The curves dail verifies signatures on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CurveId {
    P256,
    P384,
}

/// ECDSA verification (SEC 1 v2 §4.1.4) of the DER signature `SEQUENCE { r, s }` (RFC 3279 §2.2.3) over `message`
/// with `hash`, for the uncompressed public key `public_key`. The hash need not match the curve's size (TLS 1.2 and
/// X.509 allow P-256 with SHA-384 and P-384 with SHA-256): the crate takes the hash's leftmost bits.
pub(crate) fn verify(
    id: CurveId,
    hash: SigHash,
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), Error> {
    // X.509 and TLS keys are uncompressed points (RFC 5480 §2.2 allows compressed, but no CA issues them and
    // webpki's other providers refuse them).
    if public_key.first() != Some(&4) {
        return Err(Error);
    }
    let digest = hash.digest(&[message]);
    match id {
        CurveId::P256 => {
            let key = p256::ecdsa::VerifyingKey::from_sec1_bytes(public_key).map_err(|_| Error)?;
            let sig = p256::ecdsa::Signature::from_der(signature).map_err(|_| Error)?;
            key.verify_prehash(&digest, &sig).map_err(|_| Error)
        }
        CurveId::P384 => {
            let key = p384::ecdsa::VerifyingKey::from_sec1_bytes(public_key).map_err(|_| Error)?;
            let sig = p384::ecdsa::Signature::from_der(signature).map_err(|_| Error)?;
            key.verify_prehash(&digest, &sig).map_err(|_| Error)
        }
    }
}

/// ECDSA signing for the test-only key provider of the handshake tests, with the crates (RFC 6979 nonces).
#[cfg(test)]
pub(crate) fn sign_for_tests(id: CurveId, hash: SigHash, secret: &[u8], message: &[u8]) -> Vec<u8> {
    use p256::ecdsa::signature::hazmat::PrehashSigner;
    let prehash = hash.digest(&[message]);
    match id {
        CurveId::P256 => {
            let key = p256::ecdsa::SigningKey::from_slice(secret).unwrap();
            let sig: p256::ecdsa::Signature = key.sign_prehash(&prehash).unwrap();
            sig.to_der().as_bytes().to_vec()
        }
        CurveId::P384 => {
            let key = p384::ecdsa::SigningKey::from_slice(secret).unwrap();
            let sig: p384::ecdsa::Signature = key.sign_prehash(&prehash).unwrap();
            sig.to_der().as_bytes().to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use proptest::prelude::*;
    use serde::Deserialize;

    use super::*;
    use crate::testutil::{Expect, hex, sig_hash, wycheproof};

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Group {
        public_key: Key,
        sha: String,
        tests: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Key {
        curve: String,
        uncompressed: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        tc_id: u32,
        msg: String,
        sig: String,
        result: Expect,
        flags: Vec<String>,
    }

    /// Runs one Wycheproof file: `valid` cases verify, `invalid` and `acceptable` ones are refused. Returns the flags
    /// of the refused `acceptable` cases.
    fn run(file: &str) -> BTreeSet<String> {
        let vectors = wycheproof::<Group>(file);
        let mut seen = 0;
        let mut refused_acceptable = BTreeSet::new();
        for group in &vectors.test_groups {
            let id = match group.public_key.curve.as_str() {
                "secp256r1" => Some(CurveId::P256),
                "secp384r1" => Some(CurveId::P384),
                _ => None,
            }
            .unwrap();
            let key = hex(&group.public_key.uncompressed);
            let hash = sig_hash(&group.sha);
            for case in &group.tests {
                seen += 1;
                let verdict = verify(id, hash, &key, &hex(&case.msg), &hex(&case.sig));
                match case.result {
                    Expect::Valid => assert_eq!(
                        verdict,
                        Ok(()),
                        "{file} case {} {:?}",
                        case.tc_id,
                        case.flags
                    ),
                    Expect::Invalid => assert_eq!(
                        verdict,
                        Err(Error),
                        "{file} case {} {:?}",
                        case.tc_id,
                        case.flags
                    ),
                    Expect::Acceptable => {
                        assert_eq!(
                            verdict,
                            Err(Error),
                            "{file} case {} {:?}",
                            case.tc_id,
                            case.flags
                        );
                        refused_acceptable.extend(case.flags.iter().cloned());
                    }
                }
            }
        }
        assert_eq!(seen, vectors.number_of_tests, "{file}");
        refused_acceptable
    }

    #[test]
    fn wycheproof_p256_sha256() {
        assert_eq!(run("ecdsa_secp256r1_sha256_test.json"), BTreeSet::new());
    }

    #[test]
    fn wycheproof_p384_sha256() {
        assert_eq!(run("ecdsa_secp384r1_sha256_test.json"), BTreeSet::new());
    }

    #[test]
    fn wycheproof_p384_sha384() {
        assert_eq!(run("ecdsa_secp384r1_sha384_test.json"), BTreeSet::new());
    }

    fn sign_with_oracle(
        id: CurveId,
        hash: SigHash,
        secret: &[u8],
        message: &[u8],
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        use p256::ecdsa::signature::hazmat::PrehashSigner;
        let prehash = hash.digest(&[message]);
        match id {
            CurveId::P256 => {
                let key = p256::ecdsa::SigningKey::from_slice(secret).ok()?;
                let sig: p256::ecdsa::Signature = key.sign_prehash(&prehash).ok()?;
                let public = key.verifying_key().to_sec1_point(false).as_bytes().to_vec();
                Some((public, sig.to_der().as_bytes().to_vec()))
            }
            CurveId::P384 => {
                let key = p384::ecdsa::SigningKey::from_slice(secret).ok()?;
                let sig: p384::ecdsa::Signature = key.sign_prehash(&prehash).ok()?;
                let public = key.verifying_key().to_sec1_point(false).as_bytes().to_vec();
                Some((public, sig.to_der().as_bytes().to_vec()))
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Signatures over any message and either hash verify through dail's glue, and a changed message or a
        /// mutated signature is refused.
        #[test]
        fn verifies_what_rustcrypto_signs(
            p384 in any::<bool>(),
            sha384 in any::<bool>(),
            secret in any::<[u8; 48]>(),
            message in proptest::collection::vec(any::<u8>(), 0..100),
            flip in any::<proptest::sample::Index>(),
        ) {
            let id = if p384 { CurveId::P384 } else { CurveId::P256 };
            let hash = if sha384 { SigHash::Sha384 } else { SigHash::Sha256 };
            let secret = &secret[..if p384 { 48 } else { 32 }];
            let Some((public, sig)) = sign_with_oracle(id, hash, secret, &message) else {
                return Ok(());
            };
            prop_assert_eq!(verify(id, hash, &public, &message, &sig), Ok(()));
            let mut other = message.clone();
            other.push(0);
            prop_assert_eq!(verify(id, hash, &public, &other, &sig), Err(Error));
            let mut mutated = sig.clone();
            mutated[flip.index(sig.len())] ^= 1;
            prop_assert_eq!(verify(id, hash, &public, &message, &mutated), Err(Error));
        }

        /// Random keys and signatures never panic the verifier.
        #[test]
        fn random_input_never_panics(
            p384 in any::<bool>(),
            key in proptest::collection::vec(any::<u8>(), 0..100),
            sig in proptest::collection::vec(any::<u8>(), 0..110),
        ) {
            let id = if p384 { CurveId::P384 } else { CurveId::P256 };
            prop_assert_eq!(verify(id, SigHash::Sha256, &key, b"m", &sig), Err(Error));
        }
    }
}
