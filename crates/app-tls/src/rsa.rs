//! RSA signature verification (RFC 8017) with the `rsa` crate's stable 0.9 line (ADR 0014): RSASSA-PKCS1-v1_5
//! (§8.2.2) and RSASSA-PSS (§8.1.2) with SHA-256/384/512. Only public keys are used: RUSTSEC-2023-0071 (Marvin) is
//! about private-key operations. Only the glue is dail's: the key limits ring sets, and the hash a scheme names.

use rsa::pkcs1::der::Decode as _;
use rsa::{BigUint, Pkcs1v15Sign, Pss, RsaPublicKey};

use crate::Error;
use crate::hash::SigHash;

/// The smallest and largest modulus accepted, in bits (ring's bounds).
const MIN_BITS: usize = 2048;
const MAX_BITS: usize = 8192;

/// An RSA public key, checked for verification.
pub(crate) struct PublicKey(RsaPublicKey);

impl PublicKey {
    /// Parses the DER `RSAPublicKey { modulus, publicExponent }` (RFC 8017 §A.1.1): the `subjectPublicKey` bytes that
    /// webpki passes. Refuses trailing bytes, a modulus outside 2048–8192 bits or even, and an even exponent or one
    /// outside 3 ≤ e < 2^33.
    pub(crate) fn from_der(der: &[u8]) -> Result<Self, Error> {
        let parsed = rsa::pkcs1::RsaPublicKey::from_der(der).map_err(|_| Error)?;
        let (n, e) = (parsed.modulus.as_bytes(), parsed.public_exponent.as_bytes());
        let e_value = match e.len() {
            1..=5 => e
                .iter()
                .fold(0u64, |acc, &byte| (acc << 8) | u64::from(byte)),
            _ => return Err(Error),
        };
        if !(3..1 << 33).contains(&e_value) || e_value & 1 == 0 {
            return Err(Error);
        }
        let odd = n.last().is_some_and(|byte| byte & 1 == 1);
        let n = BigUint::from_bytes_be(n);
        let bits = n.bits();
        if !(MIN_BITS..=MAX_BITS).contains(&bits) || !odd {
            return Err(Error);
        }
        RsaPublicKey::new_with_max_size(n, BigUint::from(e_value), MAX_BITS)
            .map(Self)
            .map_err(|_| Error)
    }

    /// RSASSA-PKCS1-v1_5-VERIFY (RFC 8017 §8.2.2) with the DigestInfo's NULL parameters required, as ring does. The
    /// crate compares the whole encoding, never parsing the padding.
    pub(crate) fn verify_pkcs1(
        &self,
        hash: SigHash,
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), Error> {
        let scheme = match hash {
            SigHash::Sha256 => Pkcs1v15Sign::new::<rsa::sha2::Sha256>(),
            SigHash::Sha384 => Pkcs1v15Sign::new::<rsa::sha2::Sha384>(),
            SigHash::Sha512 => Pkcs1v15Sign::new::<rsa::sha2::Sha512>(),
        };
        self.check(scheme, hash, message, signature)
    }

    /// RSASSA-PSS-VERIFY (RFC 8017 §8.1.2) with MGF1 over the same hash and a salt as long as the hash (RFC 8446
    /// §4.2.3).
    pub(crate) fn verify_pss(
        &self,
        hash: SigHash,
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), Error> {
        let scheme = match hash {
            SigHash::Sha256 => Pss::new_with_salt::<rsa::sha2::Sha256>(hash.len()),
            SigHash::Sha384 => Pss::new_with_salt::<rsa::sha2::Sha384>(hash.len()),
            SigHash::Sha512 => Pss::new_with_salt::<rsa::sha2::Sha512>(hash.len()),
        };
        self.check(scheme, hash, message, signature)
    }

    /// A signature must be exactly as long as the modulus (RFC 8017 §8.1.2 step 1, §8.2.2 step 1).
    fn check(
        &self,
        scheme: impl rsa::traits::SignatureScheme,
        hash: SigHash,
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), Error> {
        if signature.len() != rsa::traits::PublicKeyParts::size(&self.0) {
            return Err(Error);
        }
        let digest = hash.digest(&[message]);
        self.0.verify(scheme, &digest, signature).map_err(|_| Error)
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
        public_key_asn: String,
        sha: String,
        tests: Vec<Case>,
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
    fn run(file: &str, pss: bool) -> BTreeSet<String> {
        let vectors = wycheproof::<Group>(file);
        let mut seen = 0;
        let mut refused_acceptable = BTreeSet::new();
        for group in &vectors.test_groups {
            let key = PublicKey::from_der(&hex(&group.public_key_asn)).unwrap();
            let hash = sig_hash(&group.sha);
            for case in &group.tests {
                seen += 1;
                let (msg, sig) = (hex(&case.msg), hex(&case.sig));
                let verdict = if pss {
                    key.verify_pss(hash, &msg, &sig)
                } else {
                    key.verify_pkcs1(hash, &msg, &sig)
                };
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

    /// Wycheproof RSASSA-PKCS1-v1_5. The only `acceptable` cases encode the DigestInfo without the NULL parameters;
    /// dail requires them, as ring does.
    #[test]
    fn wycheproof_pkcs1() {
        for file in [
            "rsa_signature_2048_sha256_test.json",
            "rsa_signature_2048_sha384_test.json",
            "rsa_signature_2048_sha512_test.json",
            "rsa_signature_3072_sha256_test.json",
            "rsa_signature_3072_sha384_test.json",
            "rsa_signature_4096_sha384_test.json",
            "rsa_signature_4096_sha512_test.json",
        ] {
            assert_eq!(
                run(file, false),
                BTreeSet::from(["MissingNull".to_string()]),
                "{file}"
            );
        }
    }

    /// Wycheproof RSASSA-PSS with MGF1 over the same hash and the salt as long as the hash.
    #[test]
    fn wycheproof_pss() {
        for file in [
            "rsa_pss_2048_sha256_mgf1_32_test.json",
            "rsa_pss_2048_sha384_mgf1_48_test.json",
            "rsa_pss_4096_sha512_mgf1_64_test.json",
        ] {
            assert_eq!(run(file, true), BTreeSet::new(), "{file}");
        }
    }

    /// A 2048-bit key from Wycheproof, and the DER around its parts for the key checks below.
    fn key_der(n: &[u8], e: &[u8]) -> Vec<u8> {
        fn int(bytes: &[u8]) -> Vec<u8> {
            let mut body = bytes.to_vec();
            if body.first().is_some_and(|&b| b & 0x80 != 0) {
                body.insert(0, 0);
            }
            tlv(0x02, &body)
        }
        fn tlv(tag: u8, body: &[u8]) -> Vec<u8> {
            let mut out = vec![tag];
            match body.len() {
                len @ 0..=0x7f => out.push(len as u8),
                len @ 0x80..=0xff => out.extend_from_slice(&[0x81, len as u8]),
                len => out.extend_from_slice(&[0x82, (len >> 8) as u8, len as u8]),
            }
            out.extend_from_slice(body);
            out
        }
        tlv(0x30, &[int(n), int(e)].concat())
    }

    #[test]
    fn refuses_weak_or_malformed_keys() {
        let mut n = vec![0xc5u8; 256];
        n[255] |= 1;
        assert!(PublicKey::from_der(&key_der(&n, &[1, 0, 1])).is_ok());
        assert!(PublicKey::from_der(&key_der(&n, &[3])).is_ok());
        assert!(PublicKey::from_der(&key_der(&n, &[1, 0xff, 0xff, 0xff, 0xff])).is_ok());
        // Exponent: even, too small, too large.
        for e in [
            &[1u8, 0, 0][..],
            &[1],
            &[2, 0, 0, 0, 1],
            &[1, 0, 0, 0, 0, 1],
        ] {
            assert!(PublicKey::from_der(&key_der(&n, e)).is_err(), "{e:?}");
        }
        // Modulus: even, 2047 bits, 8193 bits.
        let mut even = n.clone();
        even[255] &= 0xfe;
        assert!(PublicKey::from_der(&key_der(&even, &[3])).is_err());
        let mut short = n.clone();
        short[0] = 0x7f;
        assert!(PublicKey::from_der(&key_der(&short, &[3])).is_err());
        let mut long = vec![1u8];
        long.extend(vec![0xc5u8; 1024]);
        long[1024] |= 1;
        assert!(PublicKey::from_der(&key_der(&long, &[3])).is_err());
        let mut max = vec![0xc5u8; 1024];
        max[1023] |= 1;
        assert!(PublicKey::from_der(&key_der(&max, &[3])).is_ok());
        // Trailing bytes.
        let mut trailing = key_der(&n, &[3]);
        trailing.push(0);
        assert!(PublicKey::from_der(&trailing).is_err());
    }

    #[test]
    fn refuses_signatures_of_the_wrong_length_or_not_below_n() {
        let mut n = vec![0xc5u8; 256];
        n[255] |= 1;
        let key = PublicKey::from_der(&key_der(&n, &[1, 0, 1])).unwrap();
        for sig in [vec![1u8; 255], vec![1u8; 257], n.clone(), vec![0xffu8; 256]] {
            assert_eq!(key.verify_pkcs1(SigHash::Sha256, b"m", &sig), Err(Error));
            assert_eq!(key.verify_pss(SigHash::Sha256, b"m", &sig), Err(Error));
        }
    }

    proptest! {
        /// Random keys and signatures never panic the parser or the verifiers.
        #[test]
        fn random_input_never_panics(der in proptest::collection::vec(any::<u8>(), 0..300), sig in proptest::collection::vec(any::<u8>(), 0..300)) {
            prop_assert!(PublicKey::from_der(&der).is_err());
            let mut n = vec![0xc5u8; 256];
            n[255] |= 1;
            let key = PublicKey::from_der(&key_der(&n, &[1, 0, 1])).unwrap();
            let mut full = sig.clone();
            full.resize(256, 0x17);
            for s in [&sig, &full] {
                prop_assert_eq!(key.verify_pkcs1(SigHash::Sha384, b"m", s), Err(Error));
                prop_assert_eq!(key.verify_pss(SigHash::Sha512, b"m", s), Err(Error));
            }
        }
    }
}
