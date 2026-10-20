//! Hash functions, all from the `sha2` crate: SHA-256 and HMAC-SHA256 for the cipher suites, and SHA-256, SHA-384
//! and SHA-512 inside signature verification.

use sha2::Digest;
use zeroize::Zeroize;

/// The hash a signature algorithm applies to the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SigHash {
    Sha256,
    Sha384,
    Sha512,
}

impl SigHash {
    /// Output length in bytes.
    pub(crate) fn len(self) -> usize {
        match self {
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }

    /// The hash of the concatenation of `parts`.
    pub(crate) fn digest(self, parts: &[&[u8]]) -> Vec<u8> {
        fn run<D: Digest>(parts: &[&[u8]]) -> Vec<u8> {
            let mut hasher = D::new();
            for part in parts {
                hasher.update(part);
            }
            hasher.finalize().to_vec()
        }
        match self {
            Self::Sha256 => run::<sha2::Sha256>(parts),
            Self::Sha384 => run::<sha2::Sha384>(parts),
            Self::Sha512 => run::<sha2::Sha512>(parts),
        }
    }
}

/// SHA-256 as the hash of the cipher suites: the handshake transcript, HKDF and the TLS 1.2 PRF.
pub(crate) struct Sha256;

impl rustls::crypto::hash::Hash for Sha256 {
    fn start(&self) -> Box<dyn rustls::crypto::hash::Context> {
        Box::new(Sha256Context(sha2::Sha256::new()))
    }

    fn hash(&self, data: &[u8]) -> rustls::crypto::hash::Output {
        rustls::crypto::hash::Output::new(&sha2::Sha256::digest(data))
    }

    fn output_len(&self) -> usize {
        32
    }

    fn algorithm(&self) -> rustls::crypto::hash::HashAlgorithm {
        rustls::crypto::hash::HashAlgorithm::SHA256
    }
}

/// A running SHA-256; `fork` clones the state.
struct Sha256Context(sha2::Sha256);

impl rustls::crypto::hash::Context for Sha256Context {
    fn fork_finish(&self) -> rustls::crypto::hash::Output {
        rustls::crypto::hash::Output::new(&self.0.clone().finalize())
    }

    fn fork(&self) -> Box<dyn rustls::crypto::hash::Context> {
        Box::new(Self(self.0.clone()))
    }

    fn finish(self: Box<Self>) -> rustls::crypto::hash::Output {
        rustls::crypto::hash::Output::new(&self.0.finalize())
    }

    fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }
}

/// The SHA-256 block size, which is also the HMAC key block size (RFC 2104 §2).
const BLOCK_LEN: usize = 64;

/// HMAC-SHA256 (RFC 2104). rustls builds HKDF (`HkdfUsingHmac`) and the TLS 1.2 PRF (`PrfUsingHmac`) on it.
pub(crate) struct HmacSha256;

impl rustls::crypto::hmac::Hmac for HmacSha256 {
    fn with_key(&self, key: &[u8]) -> Box<dyn rustls::crypto::hmac::Key> {
        Box::new(HmacKey::new(key))
    }

    fn hash_output_len(&self) -> usize {
        32
    }
}

/// An HMAC key as its 64-byte block: the key itself, or its SHA-256 if it is longer than a block, padded with zeros
/// (RFC 2104 §2). Wiped on drop.
pub(crate) struct HmacKey([u8; BLOCK_LEN]);

impl HmacKey {
    pub(crate) fn new(key: &[u8]) -> Self {
        let mut block = [0u8; BLOCK_LEN];
        if key.len() > BLOCK_LEN {
            let mut digest: [u8; 32] = sha2::Sha256::digest(key).into();
            block[..32].copy_from_slice(&digest);
            digest.zeroize();
        } else {
            block[..key.len()].copy_from_slice(key);
        }
        Self(block)
    }

    /// `H(K ⊕ opad ‖ H(K ⊕ ipad ‖ message))` over the concatenation of `parts`.
    pub(crate) fn sign(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut pad = self.0.map(|byte| byte ^ 0x36);
        let mut inner = sha2::Sha256::new();
        inner.update(pad);
        for part in parts {
            inner.update(part);
        }
        let mut inner_hash: [u8; 32] = inner.finalize().into();
        pad = self.0.map(|byte| byte ^ 0x5c);
        let mut outer = sha2::Sha256::new();
        outer.update(pad);
        outer.update(inner_hash);
        pad.zeroize();
        inner_hash.zeroize();
        outer.finalize().into()
    }
}

impl rustls::crypto::hmac::Key for HmacKey {
    fn sign_concat(
        &self,
        first: &[u8],
        middle: &[&[u8]],
        last: &[u8],
    ) -> rustls::crypto::hmac::Tag {
        let mut parts = Vec::with_capacity(middle.len() + 2);
        parts.push(first);
        parts.extend_from_slice(middle);
        parts.push(last);
        rustls::crypto::hmac::Tag::new(&self.sign(&parts))
    }

    fn tag_len(&self) -> usize {
        32
    }
}

impl Drop for HmacKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use rustls::crypto::hash::Hash as _;
    use rustls::crypto::tls13::{Hkdf, HkdfUsingHmac};
    use serde::Deserialize;

    use super::*;
    use crate::testutil::{Expect, hex, wycheproof};

    fn hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
        HmacKey::new(key).sign(&[data])
    }

    /// RFC 4231 §4.2–4.8, HMAC-SHA-256. Case 5 checks the 128-bit truncation; cases 6 and 7 hash a long key.
    #[test]
    fn rfc4231() {
        let cases: [(Vec<u8>, &[u8], &str); 7] = [
            (vec![0x0b; 20], b"Hi There", "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"),
            (b"Jefe".to_vec(), b"what do ya want for nothing?", "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"),
            (vec![0xaa; 20], &[0xdd; 50], "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"),
            ((1..=25).collect(), &[0xcd; 50], "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b"),
            (vec![0x0c; 20], b"Test With Truncation", "a3b6167473100ee06e0c796c2955552b"),
            (vec![0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First", "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"),
            (
                vec![0xaa; 131],
                b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm.",
                "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2",
            ),
        ];
        for (n, (key, data, tag)) in cases.iter().enumerate() {
            let expected = hex(tag);
            assert_eq!(
                hmac(key, data)[..expected.len()],
                expected[..],
                "case {}",
                n + 1
            );
        }
    }

    #[test]
    fn sign_concat_joins_its_parts() {
        let key = HmacKey::new(b"key");
        let joined = rustls::crypto::hmac::Key::sign_concat(&key, b"ab", &[b"cd", b"", b"e"], b"f");
        assert_eq!(joined.as_ref(), hmac(b"key", b"abcdef"));
    }

    #[test]
    fn sha256_contexts_fork() {
        let mut ctx = Sha256.start();
        ctx.update(b"ab");
        let fork = ctx.fork();
        ctx.update(b"c");
        assert_eq!(ctx.fork_finish().as_ref(), Sha256.hash(b"abc").as_ref());
        assert_eq!(fork.finish().as_ref(), Sha256.hash(b"ab").as_ref());
        let expected = hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(ctx.finish().as_ref(), expected);
    }

    /// RFC 5869 A.1–A.3 through rustls's `HkdfUsingHmac` over this HMAC.
    #[test]
    fn rfc5869_through_rustls_hkdf() {
        let range = |from: u8, to: u8| (from..=to).collect::<Vec<u8>>();
        let cases = [
            (
                vec![0x0b; 22],
                range(0x00, 0x0c),
                range(0xf0, 0xf9),
                "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865",
            ),
            (
                range(0x00, 0x4f),
                range(0x60, 0xaf),
                range(0xb0, 0xff),
                concat!(
                    "b11e398dc80327a1c8e7f78c596a49344f012eda2d4efad8a050cc4c19afa97c59045a99cac7827271cb41c65e590e09",
                    "da3275600c2f09b8367793a9aca3db71cc30c58179ec3e87c14c01d5c1f3434f1d87",
                ),
            ),
            (
                vec![0x0b; 22],
                vec![],
                vec![],
                "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8",
            ),
        ];
        for (n, (ikm, salt, info, okm)) in cases.iter().enumerate() {
            let expander = HkdfUsingHmac(&HmacSha256).extract_from_secret(Some(salt), ikm);
            let mut out = vec![0u8; okm.len() / 2];
            expander.expand_slice(&[info], &mut out).unwrap();
            assert_eq!(out, hex(okm), "case {}", n + 1);
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Group {
        tag_size: usize,
        tests: Vec<Case>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        tc_id: u32,
        key: String,
        msg: String,
        tag: String,
        result: Expect,
    }

    /// Wycheproof `hmac_sha256_test.json`, including the groups with truncated tags.
    #[test]
    fn wycheproof_hmac_sha256() {
        let file = wycheproof::<Group>("hmac_sha256_test.json");
        let mut seen = 0;
        for group in &file.test_groups {
            for case in &group.tests {
                seen += 1;
                let tag = hmac(&hex(&case.key), &hex(&case.msg));
                let matches = crate::ct::eq(&tag[..group.tag_size / 8], &hex(&case.tag));
                assert_eq!(matches, case.result == Expect::Valid, "case {}", case.tc_id);
            }
        }
        assert_eq!(seen, file.number_of_tests);
    }
}
