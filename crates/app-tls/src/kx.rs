//! Key exchange groups (RFC 8446 §4.2.7, §7.4): X25519 from `x25519-dalek` (RFC 7748) and the NIST curves P-256 and
//! P-384 from `p256` and `p384` (ADR 0014). Only the TLS framing is dail's: key shares, point encoding and the
//! checks RFC 8446 asks for.

use rustls::crypto::{ActiveKeyExchange, GetRandomFailed, SharedSecret, SupportedKxGroup};
use rustls::{NamedGroup, PeerMisbehaved};
use zeroize::Zeroize;

use crate::Error;

/// X25519 key and u-coordinate length in bytes.
pub(crate) const LEN: usize = 32;

/// The X25519 base point, u = 9 (RFC 7748 §4.1).
#[cfg(test)]
pub(crate) const BASE_POINT: [u8; LEN] = x25519_dalek::X25519_BASEPOINT_BYTES;

/// The X25519 function (RFC 7748 §5): the u-coordinate of `scalar · u`, with the scalar clamped.
#[cfg(test)]
pub(crate) fn x25519(scalar: &[u8; LEN], u: &[u8; LEN]) -> [u8; LEN] {
    x25519_dalek::x25519(*scalar, *u)
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

    fn secret(&self) -> x25519_dalek::StaticSecret {
        x25519_dalek::StaticSecret::from(self.0)
    }

    /// The public key, `X25519(key, 9)` (RFC 7748 §6.1).
    pub(crate) fn public_key(&self) -> [u8; LEN] {
        x25519_dalek::PublicKey::from(&self.secret()).to_bytes()
    }

    /// The shared secret with the peer's public key `peer` (RFC 7748 §6.1). Refuses a peer key that isn't exactly 32
    /// bytes, and an all-zero secret (a low-order peer key; RFC 8446 §7.4.2), checked in constant time by
    /// `was_contributory`.
    pub(crate) fn agree(&self, peer: &[u8]) -> Result<[u8; LEN], Error> {
        let peer: [u8; LEN] = peer.try_into().map_err(|_| Error)?;
        let shared = self
            .secret()
            .diffie_hellman(&x25519_dalek::PublicKey::from(peer));
        if !shared.was_contributory() {
            return Err(Error);
        }
        Ok(shared.to_bytes())
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
        Ok(Box::new(X25519Exchange { key, public }))
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::X25519
    }
}

/// One handshake's X25519 key pair. The private key is wiped when the exchange completes or is dropped.
struct X25519Exchange {
    key: PrivateKey,
    public: [u8; LEN],
}

impl ActiveKeyExchange for X25519Exchange {
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

/// A NIST curve as a TLS key exchange group: ECDHE with uncompressed points (RFC 8446 §4.2.8.2, RFC 8422 §5.4.1).
#[derive(Debug)]
pub(crate) struct Nist(Curve);

#[derive(Debug, Clone, Copy)]
enum Curve {
    P256,
    P384,
}

pub(crate) static SECP256R1: Nist = Nist(Curve::P256);
pub(crate) static SECP384R1: Nist = Nist(Curve::P384);

impl SupportedKxGroup for Nist {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, rustls::Error> {
        Ok(match self.0 {
            Curve::P256 => Box::new(NistExchange::<p256::NistP256>::generate(
                NamedGroup::secp256r1,
            )?),
            Curve::P384 => Box::new(NistExchange::<p384::NistP384>::generate(
                NamedGroup::secp384r1,
            )?),
        })
    }

    fn name(&self) -> NamedGroup {
        match self.0 {
            Curve::P256 => NamedGroup::secp256r1,
            Curve::P384 => NamedGroup::secp384r1,
        }
    }
}

/// One handshake's key pair on curve `C`; the secret key zeroizes itself on drop.
struct NistExchange<C: p256::elliptic_curve::CurveArithmetic> {
    group: NamedGroup,
    secret: p256::elliptic_curve::SecretKey<C>,
    public: Vec<u8>,
}

impl<C> NistExchange<C>
where
    C: p256::elliptic_curve::CurveArithmetic,
    p256::elliptic_curve::AffinePoint<C>:
        p256::elliptic_curve::sec1::FromSec1Point<C> + p256::elliptic_curve::sec1::ToSec1Point<C>,
    p256::elliptic_curve::FieldBytesSize<C>: p256::elliptic_curve::sec1::ModulusSize,
{
    /// A fresh key: random bytes from the OS until they are a valid scalar (all but a negligible share are).
    fn generate(group: NamedGroup) -> Result<Self, rustls::Error> {
        let mut bytes = p256::elliptic_curve::FieldBytes::<C>::default();
        for _ in 0..16 {
            getrandom::fill(&mut bytes).map_err(|_| GetRandomFailed)?;
            let made = p256::elliptic_curve::SecretKey::<C>::from_bytes(&bytes);
            bytes.zeroize();
            if let Ok(secret) = made {
                let public = p256::elliptic_curve::sec1::ToSec1Point::to_sec1_point(
                    secret.public_key().as_affine(),
                    false,
                )
                .as_bytes()
                .to_vec();
                return Ok(Self {
                    group,
                    secret,
                    public,
                });
            }
        }
        Err(GetRandomFailed.into())
    }
}

impl<C> ActiveKeyExchange for NistExchange<C>
where
    C: p256::elliptic_curve::CurveArithmetic,
    p256::elliptic_curve::AffinePoint<C>:
        p256::elliptic_curve::sec1::FromSec1Point<C> + p256::elliptic_curve::sec1::ToSec1Point<C>,
    p256::elliptic_curve::FieldBytesSize<C>: p256::elliptic_curve::sec1::ModulusSize,
{
    /// Refuses a peer key that is not an uncompressed point on the curve (RFC 8446 §4.2.8.2 forbids the compressed
    /// form; the crate checks the point is on the curve and not the identity).
    fn complete(self: Box<Self>, peer: &[u8]) -> Result<SharedSecret, rustls::Error> {
        if peer.first() != Some(&4) {
            return Err(PeerMisbehaved::InvalidKeyShare.into());
        }
        let peer = p256::elliptic_curve::PublicKey::<C>::from_sec1_bytes(peer)
            .map_err(|_| PeerMisbehaved::InvalidKeyShare)?;
        let shared = p256::elliptic_curve::ecdh::diffie_hellman(
            self.secret.to_nonzero_scalar(),
            peer.as_affine(),
        );
        Ok(SharedSecret::from(shared.raw_secret_bytes().as_slice()))
    }

    fn pub_key(&self) -> &[u8] {
        &self.public
    }

    fn group(&self) -> NamedGroup {
        self.group
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

        /// Both sides of an X25519 agreement compute the same secret.
        #[test]
        fn diffie_hellman_agrees(a in any::<[u8; LEN]>(), b in any::<[u8; LEN]>()) {
            let (a, b) = (PrivateKey(a), PrivateKey(b));
            prop_assert_eq!(a.agree(&b.public_key()).unwrap(), b.agree(&a.public_key()).unwrap());
        }
    }

    /// Both sides of a P-256 and a P-384 exchange through rustls's interface agree, and a malformed or compressed
    /// peer key is refused.
    #[test]
    fn nist_groups_agree_and_refuse_bad_points() {
        for group in [&SECP256R1 as &dyn SupportedKxGroup, &SECP384R1] {
            let (a, b) = (group.start().unwrap(), group.start().unwrap());
            assert_eq!(a.pub_key()[0], 4, "{:?}: uncompressed", group.name());
            let (a_public, b_public) = (a.pub_key().to_vec(), b.pub_key().to_vec());
            let ab = a.complete(&b_public).unwrap();
            let ba = b.complete(&a_public).unwrap();
            assert_eq!(ab.secret_bytes(), ba.secret_bytes());
            let mut off_curve = a_public.clone();
            let last = off_curve.len() - 1;
            off_curve[last] ^= 1;
            assert!(group.start().unwrap().complete(&off_curve).is_err());
            let mut compressed = a_public[..a_public.len().div_ceil(2)].to_vec();
            compressed[0] = 2;
            assert!(group.start().unwrap().complete(&compressed).is_err());
            assert!(group.start().unwrap().complete(&[4]).is_err());
        }
    }
}
