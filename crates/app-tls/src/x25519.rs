//! X25519 key agreement (RFC 7748 §5): the Montgomery ladder over GF(2^255 − 19), with field elements in five 51-bit
//! limbs and `u128` products (the layout of curve25519-donna-c64). The ladder runs 255 steps whatever the scalar, and
//! nothing branches on, or indexes with, the scalar or a field value.

use rustls::crypto::{ActiveKeyExchange, GetRandomFailed, SharedSecret, SupportedKxGroup};
use rustls::{NamedGroup, PeerMisbehaved};
use zeroize::Zeroize;

use crate::{Error, ct};

/// Bytes in a scalar, a u-coordinate and a shared secret.
pub(crate) const LEN: usize = 32;

/// The u-coordinate of the base point, 9 (RFC 7748 §4.1).
pub(crate) const BASE_POINT: [u8; LEN] = {
    let mut u = [0u8; LEN];
    u[0] = 9;
    u
};

const MASK51: u64 = (1 << 51) - 1;

/// An element of GF(2^255 − 19): Σ limb[i]·2^(51·i). The limbs may exceed 51 bits by a little between operations.
type Fe = [u64; 5];

const ONE: Fe = [1, 0, 0, 0, 0];
const ZERO: Fe = [0; 5];

fn load64(bytes: &[u8; LEN], at: usize) -> u64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(word)
}

/// Decodes a u-coordinate: little-endian, with the top bit masked (RFC 7748 §5). A non-canonical value (≥ p) is kept
/// as it is and reduces in the arithmetic.
fn fe_from_bytes(bytes: &[u8; LEN]) -> Fe {
    [
        load64(bytes, 0) & MASK51,
        (load64(bytes, 6) >> 3) & MASK51,
        (load64(bytes, 12) >> 6) & MASK51,
        (load64(bytes, 19) >> 1) & MASK51,
        (load64(bytes, 24) >> 12) & MASK51,
    ]
}

/// Carries every limb into the next and folds the top carry back times 19 (2^255 ≡ 19): afterwards every limb is
/// below 2^51 except limb 0, which may exceed it by 19 times a small carry.
fn fe_carry(mut f: Fe) -> Fe {
    for i in 0..4 {
        f[i + 1] += f[i] >> 51;
        f[i] &= MASK51;
    }
    f[0] += 19 * (f[4] >> 51);
    f[4] &= MASK51;
    f
}

/// Encodes `f` canonically: fully reduced mod p, little-endian (RFC 7748 §5).
fn fe_to_bytes(f: &Fe) -> [u8; LEN] {
    let mut t = fe_carry(fe_carry(*f));
    // q = 1 when t ≥ p, i.e. when t + 19 reaches 2^255; then t − p = t + 19 − 2^255.
    let mut q = (t[0] + 19) >> 51;
    for limb in &t[1..] {
        q = (limb + q) >> 51;
    }
    t[0] += 19 * q;
    for i in 0..4 {
        t[i + 1] += t[i] >> 51;
        t[i] &= MASK51;
    }
    t[4] &= MASK51;
    let words = [
        t[0] | (t[1] << 51),
        (t[1] >> 13) | (t[2] << 38),
        (t[2] >> 26) | (t[3] << 25),
        (t[3] >> 39) | (t[4] << 12),
    ];
    let mut out = [0u8; LEN];
    for (chunk, word) in out.as_chunks_mut::<8>().0.iter_mut().zip(words) {
        *chunk = word.to_le_bytes();
    }
    t.zeroize();
    out
}

fn fe_add(f: &Fe, g: &Fe) -> Fe {
    fe_carry(core::array::from_fn(|i| f[i] + g[i]))
}

/// `f − g`, computed as `f + 2p − g` so no limb goes negative. Both inputs have limbs below 2^52.
fn fe_sub(f: &Fe, g: &Fe) -> Fe {
    const TWO_P: Fe = [
        0xf_ffff_ffff_ffda,
        0xf_ffff_ffff_fffe,
        0xf_ffff_ffff_fffe,
        0xf_ffff_ffff_fffe,
        0xf_ffff_ffff_fffe,
    ];
    fe_carry(core::array::from_fn(|i| f[i] + TWO_P[i] - g[i]))
}

/// Carries the five `u128` column sums of a product into a field element.
fn fe_reduce_wide(mut r: [u128; 5]) -> Fe {
    for i in 0..4 {
        r[i + 1] += r[i] >> 51;
        r[i] &= u128::from(MASK51);
    }
    let mut f: Fe = core::array::from_fn(|i| r[i] as u64);
    f[0] += 19 * ((r[4] >> 51) as u64);
    f[4] &= MASK51;
    r.zeroize();
    fe_carry(f)
}

/// `f · g` mod p: schoolbook, with the limbs that overflow 2^255 folded back times 19.
fn fe_mul(f: &Fe, g: &Fe) -> Fe {
    let wide = |a: u64, b: u64| u128::from(a) * u128::from(b);
    let [f0, f1, f2, f3, f4] = *f;
    let [g0, g1, g2, g3, g4] = *g;
    let (g1_19, g2_19, g3_19, g4_19) = (g1 * 19, g2 * 19, g3 * 19, g4 * 19);
    fe_reduce_wide([
        wide(f0, g0) + wide(f1, g4_19) + wide(f2, g3_19) + wide(f3, g2_19) + wide(f4, g1_19),
        wide(f0, g1) + wide(f1, g0) + wide(f2, g4_19) + wide(f3, g3_19) + wide(f4, g2_19),
        wide(f0, g2) + wide(f1, g1) + wide(f2, g0) + wide(f3, g4_19) + wide(f4, g3_19),
        wide(f0, g3) + wide(f1, g2) + wide(f2, g1) + wide(f3, g0) + wide(f4, g4_19),
        wide(f0, g4) + wide(f1, g3) + wide(f2, g2) + wide(f3, g1) + wide(f4, g0),
    ])
}

fn fe_sq(f: &Fe) -> Fe {
    fe_mul(f, f)
}

/// `f` squared `n` times.
fn fe_sq_n(f: &Fe, n: usize) -> Fe {
    let mut r = *f;
    for _ in 0..n {
        r = fe_sq(&r);
    }
    r
}

/// `f · k` for a small constant `k`.
fn fe_mul_small(f: &Fe, k: u64) -> Fe {
    fe_reduce_wide(core::array::from_fn(|i| u128::from(f[i]) * u128::from(k)))
}

/// `z^(p − 2)` = `z^(2^255 − 21)`, the inverse of `z` by Fermat (0 for 0). The addition chain of ref10; the exponent
/// is public, the steps don't depend on `z`.
fn fe_invert(z: &Fe) -> Fe {
    let z2 = fe_sq(z);
    let z9 = fe_mul(&fe_sq_n(&z2, 2), z);
    let z11 = fe_mul(&z9, &z2);
    let z_5_0 = fe_mul(&fe_sq(&z11), &z9); // z^(2^5 − 1)
    let z_10_0 = fe_mul(&fe_sq_n(&z_5_0, 5), &z_5_0);
    let z_20_0 = fe_mul(&fe_sq_n(&z_10_0, 10), &z_10_0);
    let z_40_0 = fe_mul(&fe_sq_n(&z_20_0, 20), &z_20_0);
    let z_50_0 = fe_mul(&fe_sq_n(&z_40_0, 10), &z_10_0);
    let z_100_0 = fe_mul(&fe_sq_n(&z_50_0, 50), &z_50_0);
    let z_200_0 = fe_mul(&fe_sq_n(&z_100_0, 100), &z_100_0);
    let z_250_0 = fe_mul(&fe_sq_n(&z_200_0, 50), &z_50_0);
    fe_mul(&fe_sq_n(&z_250_0, 5), &z11) // z^(2^255 − 32 + 11)
}

/// The X25519 function (RFC 7748 §5): the u-coordinate of `scalar · u`. The scalar is clamped; the top bit of `u` is
/// masked and a non-canonical `u` is reduced; the result is encoded canonically.
pub(crate) fn x25519(scalar: &[u8; LEN], u: &[u8; LEN]) -> [u8; LEN] {
    let mut k = *scalar;
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;

    let x1 = fe_from_bytes(u);
    let (mut x2, mut z2, mut x3, mut z3) = (ONE, ZERO, x1, ONE);
    let mut swap = 0u64;
    for t in (0..255).rev() {
        let bit = u64::from((k[t / 8] >> (t % 8)) & 1);
        swap ^= bit;
        let mask = ct::mask(swap);
        ct::swap(mask, &mut x2, &mut x3);
        ct::swap(mask, &mut z2, &mut z3);
        swap = bit;

        let a = fe_add(&x2, &z2);
        let aa = fe_sq(&a);
        let b = fe_sub(&x2, &z2);
        let bb = fe_sq(&b);
        let e = fe_sub(&aa, &bb);
        let c = fe_add(&x3, &z3);
        let d = fe_sub(&x3, &z3);
        let da = fe_mul(&d, &a);
        let cb = fe_mul(&c, &b);
        x3 = fe_sq(&fe_add(&da, &cb));
        z3 = fe_mul(&x1, &fe_sq(&fe_sub(&da, &cb)));
        x2 = fe_mul(&aa, &bb);
        z2 = fe_mul(&e, &fe_add(&aa, &fe_mul_small(&e, 121_665)));
    }
    let mask = ct::mask(swap);
    ct::swap(mask, &mut x2, &mut x3);
    ct::swap(mask, &mut z2, &mut z3);

    let out = fe_to_bytes(&fe_mul(&x2, &fe_invert(&z2)));
    k.zeroize();
    for fe in [&mut x2, &mut z2, &mut x3, &mut z3] {
        fe.zeroize();
    }
    out
}

/// An X25519 private key: 32 bytes from the OS random source, wiped on drop.
pub(crate) struct PrivateKey([u8; LEN]);

impl PrivateKey {
    /// A fresh key from the OS random source.
    pub(crate) fn generate() -> Result<Self, Error> {
        let mut key = [0u8; LEN];
        getrandom::fill(&mut key).map_err(|_| Error)?;
        Ok(Self(key))
    }

    /// The public key, `X25519(key, 9)` (RFC 7748 §6.1).
    pub(crate) fn public_key(&self) -> [u8; LEN] {
        x25519(&self.0, &BASE_POINT)
    }

    /// The shared secret with the peer's public key `peer` (RFC 7748 §6.1). Refuses a peer key that isn't exactly 32
    /// bytes, and an all-zero secret (a low-order peer key; RFC 8446 §7.4.2), checked in constant time.
    pub(crate) fn agree(&self, peer: &[u8]) -> Result<[u8; LEN], Error> {
        let peer: &[u8; LEN] = peer.try_into().map_err(|_| Error)?;
        let mut shared = x25519(&self.0, peer);
        if ct::eq(&shared, &[0u8; LEN]) {
            shared.zeroize();
            return Err(Error);
        }
        Ok(shared)
    }
}

impl Drop for PrivateKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// X25519 as a TLS key exchange group (RFC 8446 §4.2.7, §7.4.2).
#[derive(Debug)]
pub(crate) struct X25519;

impl SupportedKxGroup for X25519 {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, rustls::Error> {
        let key = PrivateKey::generate().map_err(|Error| GetRandomFailed)?;
        let public = key.public_key();
        Ok(Box::new(KeyExchange { key, public }))
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::X25519
    }
}

/// One handshake's key pair. The private key is wiped when the exchange completes or is dropped.
struct KeyExchange {
    key: PrivateKey,
    public: [u8; LEN],
}

impl ActiveKeyExchange for KeyExchange {
    /// Refuses a peer key that isn't 32 bytes and an all-zero shared secret (RFC 8446 §7.4.2).
    fn complete(self: Box<Self>, peer: &[u8]) -> Result<SharedSecret, rustls::Error> {
        let mut shared = self
            .key
            .agree(peer)
            .map_err(|Error| PeerMisbehaved::InvalidKeyShare)?;
        let secret = SharedSecret::from(&shared[..]);
        shared.zeroize();
        Ok(secret)
    }

    fn pub_key(&self) -> &[u8] {
        &self.public
    }

    fn group(&self) -> NamedGroup {
        NamedGroup::X25519
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde::Deserialize;

    use super::*;
    use crate::testutil::{Expect, hex, hex_array, wycheproof};

    #[test]
    fn rfc7748_5_2_vectors() {
        let cases = [
            (
                "a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4",
                "e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c",
                "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552",
            ),
            (
                "4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d",
                "e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493",
                "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957",
            ),
        ];
        for (scalar, u, out) in cases {
            assert_eq!(x25519(&hex_array(scalar), &hex_array(u)).to_vec(), hex(out));
        }
    }

    /// RFC 7748 §5.2: k = X25519(k, u), u = old k, starting from k = u = 9.
    fn iterate(rounds: usize) -> [u8; LEN] {
        let (mut k, mut u) = (BASE_POINT, BASE_POINT);
        for _ in 0..rounds {
            let next = x25519(&k, &u);
            u = k;
            k = next;
        }
        k
    }

    #[test]
    fn rfc7748_5_2_iterated() {
        let one = "422c8e7a6227d7bca1350b3e2bb7279f7897b87bb6854b783c60e80311ae3079";
        let thousand = "684cf59ba83309552800ef566f2f4d3c1c3887c49360e3875f2eb94d99532c51";
        assert_eq!(iterate(1).to_vec(), hex(one));
        assert_eq!(iterate(1_000).to_vec(), hex(thousand));
    }

    #[test]
    #[ignore = "about 100 s; run with --ignored"]
    fn rfc7748_5_2_iterated_a_million_times() {
        let million = "7c3911e0ab2586fd864497297e575e6f3bc601c0883c30df5f4dd2d24f665424";
        assert_eq!(iterate(1_000_000).to_vec(), hex(million));
    }

    #[test]
    fn rfc7748_6_1_diffie_hellman() {
        let alice = PrivateKey(hex_array(
            "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
        ));
        let bob = PrivateKey(hex_array(
            "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb",
        ));
        let alice_public = alice.public_key();
        let bob_public = bob.public_key();
        assert_eq!(
            alice_public.to_vec(),
            hex("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a")
        );
        assert_eq!(
            bob_public.to_vec(),
            hex("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f")
        );
        let shared = hex("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
        assert_eq!(alice.agree(&bob_public).unwrap().to_vec(), shared);
        assert_eq!(bob.agree(&alice_public).unwrap().to_vec(), shared);
    }

    #[test]
    fn peer_keys_of_the_wrong_length_or_low_order_are_refused() {
        let key = PrivateKey::generate().unwrap();
        assert_eq!(key.agree(&[9u8; 31]).err(), Some(Error));
        assert_eq!(key.agree(&[9u8; 33]).err(), Some(Error));
        // u = 0 and u = 1 have small order: the shared secret is zero.
        assert_eq!(key.agree(&[0u8; LEN]).err(), Some(Error));
        let mut one = [0u8; LEN];
        one[0] = 1;
        assert_eq!(key.agree(&one).err(), Some(Error));
    }

    #[test]
    fn generated_keys_differ() {
        let (a, b) = (
            PrivateKey::generate().unwrap(),
            PrivateKey::generate().unwrap(),
        );
        assert_ne!(a.0, b.0);
        assert_eq!(
            a.agree(&b.public_key()).unwrap(),
            b.agree(&a.public_key()).unwrap()
        );
    }

    #[derive(Deserialize)]
    struct Group {
        tests: Vec<Case>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        tc_id: u32,
        private: String,
        public: String,
        shared: String,
        result: Expect,
        flags: Vec<String>,
    }

    /// Wycheproof `x25519_test.json`. Every case computes the expected u-coordinate. `agree` refuses exactly the
    /// cases with an all-zero secret (`ZeroSharedSecret`, all `acceptable`). The other `acceptable` cases (points on
    /// the twist, non-canonical u) are accepted: RFC 7748 §5 defines their result and §7 recommends accepting them,
    /// and `docs/plan-tls.md` §5.2 asks for that.
    #[test]
    fn wycheproof_x25519() {
        let file = wycheproof::<Group>("x25519_test.json");
        let mut seen = 0;
        let mut accepted_flags = std::collections::BTreeSet::new();
        for case in file.test_groups.iter().flat_map(|group| &group.tests) {
            seen += 1;
            let key = PrivateKey(hex_array(&case.private));
            let public = hex(&case.public);
            let raw = x25519(&key.0, &public.clone().try_into().unwrap());
            assert_eq!(raw.to_vec(), hex(&case.shared), "case {}", case.tc_id);
            let zero = case.flags.iter().any(|flag| flag == "ZeroSharedSecret");
            assert_eq!(zero, raw == [0u8; LEN], "case {}", case.tc_id);
            match key.agree(&public) {
                Ok(shared) => {
                    assert!(!zero, "case {}", case.tc_id);
                    assert_eq!(shared, raw);
                    if case.result == Expect::Acceptable {
                        accepted_flags.extend(case.flags.iter().cloned());
                    }
                }
                Err(Error) => assert!(zero, "case {} refused", case.tc_id),
            }
            assert_ne!(
                case.result,
                Expect::Invalid,
                "case {}: the file has no invalid cases",
                case.tc_id
            );
        }
        assert_eq!(seen, file.number_of_tests);
        let expected = [
            "EdgeCaseMultiplication",
            "EdgeCaseShared",
            "Ktv",
            "NonCanonicalPublic",
            "SpecialPublicKey",
            "Twist",
        ];
        assert_eq!(accepted_flags.into_iter().collect::<Vec<_>>(), expected);
    }

    #[test]
    fn non_canonical_u_reduces() {
        // u = p + 9 is 9; the top bit is ignored.
        let mut u =
            hex_array::<LEN>("edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f");
        u[0] += 9;
        let scalar = [0x42u8; LEN];
        assert_eq!(x25519(&scalar, &u), x25519(&scalar, &BASE_POINT));
        let mut high = BASE_POINT;
        high[31] |= 0x80;
        assert_eq!(x25519(&scalar, &high), x25519(&scalar, &BASE_POINT));
    }

    proptest! {
        /// Cross-check against `x25519-dalek` (`docs/plan-tls.md` §7.2).
        #[test]
        fn matches_x25519_dalek(scalar in any::<[u8; LEN]>(), u in any::<[u8; LEN]>()) {
            prop_assert_eq!(x25519(&scalar, &u), x25519_dalek::x25519(scalar, u));
        }

        /// Both sides of a key agreement compute the same secret.
        #[test]
        fn diffie_hellman_agrees(a in any::<[u8; LEN]>(), b in any::<[u8; LEN]>()) {
            let (a, b) = (PrivateKey(a), PrivateKey(b));
            prop_assert_eq!(a.agree(&b.public_key()).unwrap(), b.agree(&a.public_key()).unwrap());
        }
    }
}
