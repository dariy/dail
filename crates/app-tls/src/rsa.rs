//! RSA signature verification (RFC 8017): RSASSA-PKCS1-v1_5 (§8.2.2) and RSASSA-PSS (§8.1.2) with SHA-256/384/512.
//! Everything here is public data, so the code may take variable time; every check must be exact.

use core::cmp::Ordering;

use crate::Error;
use crate::bignum::{self, Modulus};
use crate::der::Reader;
use crate::hash::SigHash;

/// The smallest and largest modulus accepted, in bits (ring's bounds).
const MIN_BITS: usize = 2048;
const MAX_BITS: usize = 8192;

/// An RSA public key, checked for verification.
pub(crate) struct PublicKey {
    n: Modulus,
    /// The modulus length in bytes, which is also the signature length.
    len: usize,
    /// The public exponent, big-endian without leading zeros.
    e: Vec<u8>,
}

impl PublicKey {
    /// Parses the DER `RSAPublicKey { modulus, publicExponent }` (RFC 8017 §A.1.1): the `subjectPublicKey` bytes that
    /// webpki passes. Refuses a modulus outside 2048–8192 bits and an even exponent or one outside 3 ≤ e < 2^33.
    pub(crate) fn from_der(der: &[u8]) -> Result<Self, Error> {
        let mut outer = Reader::new(der);
        let mut seq = outer.sequence()?;
        let n = seq.unsigned_integer()?;
        let e = seq.unsigned_integer()?;
        seq.finish()?;
        outer.finish()?;

        let e_value = match e.len() {
            1..=5 => e
                .iter()
                .fold(0u64, |acc, &byte| (acc << 8) | u64::from(byte)),
            _ => return Err(Error),
        };
        if !(3..1 << 33).contains(&e_value) || e_value & 1 == 0 {
            return Err(Error);
        }
        // An even modulus fails here too: Montgomery arithmetic needs it odd, and no RSA modulus is even.
        let modulus = Modulus::new(n)?;
        if !(MIN_BITS..=MAX_BITS).contains(&modulus.bits()) {
            return Err(Error);
        }
        Ok(Self {
            len: n.len(),
            n: modulus,
            e: e.to_vec(),
        })
    }

    /// The modulus length in bits.
    fn bits(&self) -> usize {
        self.n.bits()
    }

    /// RSAVP1 (RFC 8017 §5.2.2) with I2OSP into the modulus length: `s^e mod n`. Refuses a signature whose length
    /// differs from the modulus length and a representative s ≥ n.
    fn public_op(&self, signature: &[u8]) -> Result<Vec<u8>, Error> {
        if signature.len() != self.len {
            return Err(Error);
        }
        let s = bignum::from_be_bytes(signature, self.n.limbs())?;
        if bignum::cmp(&s, self.n.n()) != Ordering::Less {
            return Err(Error);
        }
        let m = self.n.to_plain(&self.n.pow(&self.n.to_mont(&s), &self.e));
        bignum::to_be_bytes(&m, self.len)
    }

    /// RSASSA-PKCS1-v1_5-VERIFY (RFC 8017 §8.2.2). Builds the expected encoding EMSA-PKCS1-v1_5 (§9.2),
    /// `00 01 FF…FF 00 ‖ DigestInfo ‖ H(message)`, and compares it with `s^e mod n` as a whole. The padding is never
    /// parsed, which is what keeps out Bleichenbacher-style forgeries.
    pub(crate) fn verify_pkcs1(
        &self,
        hash: SigHash,
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), Error> {
        let em = self.public_op(signature)?;
        if em == pkcs1_encoding(hash, message, self.len)? {
            Ok(())
        } else {
            Err(Error)
        }
    }

    /// RSASSA-PSS-VERIFY (RFC 8017 §8.1.2) with MGF1 over the same hash and a salt as long as the hash (RFC 8446
    /// §4.2.3), EMSA-PSS-VERIFY in §9.1.2.
    pub(crate) fn verify_pss(
        &self,
        hash: SigHash,
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), Error> {
        let m = self.public_op(signature)?;
        // §8.1.2 step 2c: EM = I2OSP(m, emLen) with emBits = modBits − 1. When modBits − 1 is a multiple of 8, emLen
        // is one byte shorter than the modulus and that byte must be zero.
        let em_bits = self.bits() - 1;
        let em_len = em_bits.div_ceil(8);
        let em = match m.split_at(m.len() - em_len) {
            (lead, em) if lead.iter().all(|&byte| byte == 0) => em,
            _ => return Err(Error),
        };

        let h_len = hash.len();
        let s_len = h_len;
        // §9.1.2 step 3.
        if em_len < h_len + s_len + 2 {
            return Err(Error);
        }
        // Step 4: the trailer.
        let (rest, trailer) = em.split_at(em_len - 1);
        if trailer != [0xbc] {
            return Err(Error);
        }
        // Step 5.
        let (masked_db, h) = rest.split_at(em_len - h_len - 1);
        // Step 6: the 8·emLen − emBits leftmost bits of maskedDB must be zero.
        let top_mask = 0xffu8 >> (8 * em_len - em_bits);
        if masked_db[0] & !top_mask != 0 {
            return Err(Error);
        }
        // Steps 7–9.
        let mut db = mgf1(hash, h, masked_db.len());
        for (d, m) in db.iter_mut().zip(masked_db) {
            *d ^= m;
        }
        db[0] &= top_mask;
        // Step 10: zero padding, then 0x01.
        let separator = em_len - h_len - s_len - 2;
        if db[..separator].iter().any(|&byte| byte != 0) || db[separator] != 0x01 {
            return Err(Error);
        }
        // Steps 11–14.
        let salt = &db[db.len() - s_len..];
        let m_hash = hash.digest(&[message]);
        let h_prime = hash.digest(&[&[0u8; 8], &m_hash, salt]);
        if h == h_prime.as_slice() {
            Ok(())
        } else {
            Err(Error)
        }
    }
}

/// EMSA-PKCS1-v1_5 (RFC 8017 §9.2): `00 01 FF…FF 00 ‖ DigestInfo ‖ H(message)` in `len` bytes, with at least eight
/// 0xFF bytes.
pub(crate) fn pkcs1_encoding(hash: SigHash, message: &[u8], len: usize) -> Result<Vec<u8>, Error> {
    let prefix = digest_info_prefix(hash);
    let t_len = prefix.len() + hash.len();
    if len < t_len + 11 {
        return Err(Error);
    }
    let mut em = Vec::with_capacity(len);
    em.extend_from_slice(&[0x00, 0x01]);
    em.resize(len - t_len - 1, 0xff);
    em.push(0x00);
    em.extend_from_slice(prefix);
    em.extend_from_slice(&hash.digest(&[message]));
    Ok(em)
}

/// MGF1 (RFC 8017 §B.2.1): `len` bytes of `H(seed ‖ counter)` for counter = 0, 1, ….
pub(crate) fn mgf1(hash: SigHash, seed: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + hash.len());
    let mut counter = 0u32;
    while out.len() < len {
        out.extend_from_slice(&hash.digest(&[seed, &counter.to_be_bytes()]));
        counter += 1;
    }
    out.truncate(len);
    out
}

/// The DER `DigestInfo` up to the hash value, with NULL parameters (RFC 8017 §9.2, note 1).
fn digest_info_prefix(hash: SigHash) -> &'static [u8] {
    match hash {
        SigHash::Sha256 => &[
            0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
            0x01, 0x05, 0x00, 0x04, 0x20,
        ],
        SigHash::Sha384 => &[
            0x30, 0x41, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
            0x02, 0x05, 0x00, 0x04, 0x30,
        ],
        SigHash::Sha512 => &[
            0x30, 0x51, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
            0x03, 0x05, 0x00, 0x04, 0x40,
        ],
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
