//! ECDSA signature verification on P-256 and P-384 (SEC 1 v2 §4.1.4; curves from SEC 2 v2 §2.4.2 and §2.5.1).
//! Points are in Jacobian coordinates with a = −3 ("dbl-2001-b" and "add-2007-bl" of the Explicit-Formulas
//! Database), the field and scalar arithmetic is Montgomery arithmetic from `bignum`. Everything here is public data
//! (keys, signatures, hashes), so the code may take variable time; every check must be exact.

use core::cmp::Ordering;

use crate::Error;
use crate::bignum::{self, Limbs, Modulus};
use crate::der::Reader;
use crate::hash::SigHash;

/// The curves dail verifies signatures on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CurveId {
    P256,
    P384,
}

/// Domain parameters as big-endian hex (SEC 2 v2 §2.4.2, §2.5.1). Both curves have a = −3 and cofactor 1.
struct Params {
    p: &'static str,
    b: &'static str,
    gx: &'static str,
    gy: &'static str,
    n: &'static str,
}

const P256: Params = Params {
    p: "ffffffff00000001000000000000000000000000ffffffffffffffffffffffff",
    b: "5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604b",
    gx: "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
    gy: "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
    n: "ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551",
};

const P384: Params = Params {
    p: concat!(
        "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe",
        "ffffffff0000000000000000ffffffff",
    ),
    b: concat!(
        "b3312fa7e23ee7e4988e056be3f82d19181d9c6efe8141120314088f5013875a",
        "c656398d8a2ed19d2a85c8edd3ec2aef",
    ),
    gx: concat!(
        "aa87ca22be8b05378eb1c71ef320ad746e1d3b628ba79b9859f741e082542a38",
        "5502f25dbf55296c3a545e3872760ab7",
    ),
    gy: concat!(
        "3617de4a96262c6f5d9e98bf9292dc29f8f41dbd289a147ce9da3113b5f0b8c0",
        "0a60b1ce1d7e819d7a431d7c90ea0e5f",
    ),
    n: concat!(
        "ffffffffffffffffffffffffffffffffffffffffffffffffc7634d81f4372ddf",
        "581a0db248b0a77aecec196accc52973",
    ),
};

/// Decodes the compile-time hex constants above.
fn unhex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |c: u8| match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                _ => 0,
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

/// A point in Jacobian coordinates (x = X/Z², y = Y/Z³), each in Montgomery form mod p. Z = 0 is the point at
/// infinity.
#[derive(Clone)]
struct Point {
    x: Limbs,
    y: Limbs,
    z: Limbs,
}

/// A curve with its moduli ready for arithmetic.
struct Curve {
    /// Field elements and scalars in bytes.
    len: usize,
    p: Modulus,
    n: Modulus,
    b: Limbs,
    g: Point,
}

impl Curve {
    fn new(id: CurveId) -> Result<Self, Error> {
        let params = match id {
            CurveId::P256 => &P256,
            CurveId::P384 => &P384,
        };
        let p = Modulus::new(&unhex(params.p))?;
        let n = Modulus::new(&unhex(params.n))?;
        let limbs = p.limbs();
        let field =
            |hex: &str| bignum::from_be_bytes(&unhex(hex), limbs).map(|value| p.to_mont(&value));
        let g = Point {
            x: field(params.gx)?,
            y: field(params.gy)?,
            z: p.one(),
        };
        Ok(Self {
            len: 8 * limbs,
            b: field(params.b)?,
            g,
            p,
            n,
        })
    }

    fn infinity(&self) -> Point {
        let zero = vec![0u64; self.p.limbs()];
        Point {
            x: self.p.one(),
            y: self.p.one(),
            z: zero,
        }
    }

    fn mul(&self, a: &[u64], b: &[u64]) -> Limbs {
        self.p.mont_mul(a, b)
    }

    fn sq(&self, a: &[u64]) -> Limbs {
        self.p.mont_mul(a, a)
    }

    fn add(&self, a: &[u64], b: &[u64]) -> Limbs {
        self.p.add(a, b)
    }

    fn sub(&self, a: &[u64], b: &[u64]) -> Limbs {
        self.p.sub(a, b)
    }

    /// Decodes an uncompressed public key `04 ‖ X ‖ Y` (SEC 1 v2 §2.3.4) and checks that X, Y < p and that the point
    /// is on the curve, y² = x³ − 3x + b (§3.2.2.1). A prime-order curve has no other points to rule out.
    fn decode_point(&self, bytes: &[u8]) -> Result<Point, Error> {
        let (&tag, coords) = bytes.split_first().ok_or(Error)?;
        if tag != 0x04 || coords.len() != 2 * self.len {
            return Err(Error);
        }
        let (x, y) = coords.split_at(self.len);
        let limbs = self.p.limbs();
        let (x, y) = (
            bignum::from_be_bytes(x, limbs)?,
            bignum::from_be_bytes(y, limbs)?,
        );
        for coordinate in [&x, &y] {
            if bignum::cmp(coordinate, self.p.n()) != Ordering::Less {
                return Err(Error);
            }
        }
        let (x, y) = (self.p.to_mont(&x), self.p.to_mont(&y));
        let three_x = self.add(&self.add(&x, &x), &x);
        let rhs = self.add(&self.sub(&self.mul(&self.sq(&x), &x), &three_x), &self.b);
        if self.sq(&y) != rhs {
            return Err(Error);
        }
        Ok(Point {
            x,
            y,
            z: self.p.one(),
        })
    }

    /// `2·P`, "dbl-2001-b" (a = −3). Infinity doubles to infinity (Z3 = 0); a prime-order curve has no point with
    /// y = 0.
    fn double(&self, pt: &Point) -> Point {
        let delta = self.sq(&pt.z);
        let gamma = self.sq(&pt.y);
        let beta = self.mul(&pt.x, &gamma);
        let t = self.mul(&self.sub(&pt.x, &delta), &self.add(&pt.x, &delta));
        let alpha = self.add(&self.add(&t, &t), &t);
        let beta2 = self.add(&beta, &beta);
        let beta4 = self.add(&beta2, &beta2);
        let beta8 = self.add(&beta4, &beta4);
        let x3 = self.sub(&self.sq(&alpha), &beta8);
        let z3 = self.sub(&self.sub(&self.sq(&self.add(&pt.y, &pt.z)), &gamma), &delta);
        let gamma_sq = self.sq(&gamma);
        let g2 = self.add(&gamma_sq, &gamma_sq);
        let g4 = self.add(&g2, &g2);
        let g8 = self.add(&g4, &g4);
        let y3 = self.sub(&self.mul(&alpha, &self.sub(&beta4, &x3)), &g8);
        Point {
            x: x3,
            y: y3,
            z: z3,
        }
    }

    /// `P + Q`, "add-2007-bl", with the cases the formula doesn't cover handled first: either input at infinity,
    /// P = Q (double instead) and P = −Q (the result is infinity).
    fn add_points(&self, p1: &Point, p2: &Point) -> Point {
        if bignum::is_zero(&p1.z) {
            return p2.clone();
        }
        if bignum::is_zero(&p2.z) {
            return p1.clone();
        }
        let z1z1 = self.sq(&p1.z);
        let z2z2 = self.sq(&p2.z);
        let u1 = self.mul(&p1.x, &z2z2);
        let u2 = self.mul(&p2.x, &z1z1);
        let s1 = self.mul(&self.mul(&p1.y, &p2.z), &z2z2);
        let s2 = self.mul(&self.mul(&p2.y, &p1.z), &z1z1);
        let h = self.sub(&u2, &u1);
        let s_diff = self.sub(&s2, &s1);
        if bignum::is_zero(&h) {
            return if bignum::is_zero(&s_diff) {
                self.double(p1)
            } else {
                self.infinity()
            };
        }
        let h2 = self.add(&h, &h);
        let i = self.sq(&h2);
        let j = self.mul(&h, &i);
        let r = self.add(&s_diff, &s_diff);
        let v = self.mul(&u1, &i);
        let x3 = self.sub(&self.sub(&self.sq(&r), &j), &self.add(&v, &v));
        let s1j = self.mul(&s1, &j);
        let y3 = self.sub(&self.mul(&r, &self.sub(&v, &x3)), &self.add(&s1j, &s1j));
        let z_sum = self.sq(&self.add(&p1.z, &p2.z));
        let z3 = self.mul(&self.sub(&self.sub(&z_sum, &z1z1), &z2z2), &h);
        Point {
            x: x3,
            y: y3,
            z: z3,
        }
    }

    /// `u1·G + u2·Q` with both scalars interleaved (Shamir's trick). The scalars are plain integers below n.
    fn double_mul(&self, u1: &[u64], u2: &[u64], q: &Point) -> Point {
        let g_plus_q = self.add_points(&self.g, q);
        let mut acc = self.infinity();
        let bit = |k: &[u64], i: usize| (k[i / 64] >> (i % 64)) & 1 == 1;
        for i in (0..bignum::bits(u1).max(bignum::bits(u2))).rev() {
            acc = self.double(&acc);
            match (bit(u1, i), bit(u2, i)) {
                (true, true) => acc = self.add_points(&acc, &g_plus_q),
                (true, false) => acc = self.add_points(&acc, &self.g),
                (false, true) => acc = self.add_points(&acc, q),
                (false, false) => {}
            }
        }
        acc
    }
}

/// A scalar in [1, n − 1] from DER INTEGER bytes, or an error.
fn scalar(curve: &Curve, bytes: &[u8]) -> Result<Limbs, Error> {
    let value = bignum::from_be_bytes(bytes, curve.n.limbs())?;
    if bignum::is_zero(&value) || bignum::cmp(&value, curve.n.n()) != Ordering::Less {
        return Err(Error);
    }
    Ok(value)
}

/// ECDSA verification (SEC 1 v2 §4.1.4) of the DER signature `SEQUENCE { r, s }` (RFC 3279 §2.2.3) over `message`
/// with `hash`, for the uncompressed public key `public_key`.
pub(crate) fn verify(
    id: CurveId,
    hash: SigHash,
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), Error> {
    let curve = Curve::new(id)?;
    let q = curve.decode_point(public_key)?;

    let mut outer = Reader::new(signature);
    let mut seq = outer.sequence()?;
    let r = scalar(&curve, seq.unsigned_integer()?)?;
    let s = scalar(&curve, seq.unsigned_integer()?)?;
    seq.finish()?;
    outer.finish()?;

    // Step 5: e = the leftmost bits of the hash, as many as n has (256 or 384, whole bytes), reduced mod n. The
    // hash is less than 2^bits(n) < 2n, so one subtraction reduces it.
    let digest = hash.digest(&[message]);
    let leftmost = &digest[..digest.len().min(curve.len)];
    let mut e = bignum::from_be_bytes(leftmost, curve.n.limbs())?;
    if bignum::cmp(&e, curve.n.n()) != Ordering::Less {
        bignum::sub_assign(&mut e, curve.n.n());
    }

    // Steps 4 and 6: w = s⁻¹, u1 = e·w, u2 = r·w (mod n).
    let n = &curve.n;
    let w = n.invert_prime(&n.to_mont(&s));
    let u1 = n.to_plain(&n.mont_mul(&n.to_mont(&e), &w));
    let u2 = n.to_plain(&n.mont_mul(&n.to_mont(&r), &w));

    // Step 7: R = u1·G + u2·Q, refused at infinity.
    let point = curve.double_mul(&u1, &u2, &q);
    if bignum::is_zero(&point.z) {
        return Err(Error);
    }
    // Step 8: x = X/Z² as an integer, then mod n (x < p < 2n, so at most one subtraction).
    let z_inv = curve.p.invert_prime(&point.z);
    let mut x = curve.p.to_plain(&curve.mul(&point.x, &curve.sq(&z_inv)));
    if bignum::cmp(&x, n.n()) != Ordering::Less {
        bignum::sub_assign(&mut x, n.n());
    }
    // Step 9.
    if x == r { Ok(()) } else { Err(Error) }
}

/// ECDSA signing (SEC 1 v2 §4.1.3) for the test-only key provider of the handshake tests: variable time, and the
/// nonce is derived from the key and the message. Never use it outside tests.
#[cfg(test)]
pub(crate) fn sign_for_tests(id: CurveId, hash: SigHash, secret: &[u8], message: &[u8]) -> Vec<u8> {
    let curve = Curve::new(id).unwrap();
    let n = &curve.n;
    let limbs = n.limbs();
    let reduce = |bytes: &[u8]| {
        let mut value = bignum::from_be_bytes(&bytes[..bytes.len().min(curve.len)], limbs).unwrap();
        if bignum::cmp(&value, n.n()) != Ordering::Less {
            bignum::sub_assign(&mut value, n.n());
        }
        value
    };
    let d = reduce(secret);
    let e = reduce(&hash.digest(&[message]));
    let k = reduce(&SigHash::Sha512.digest(&[secret, message]));
    let zero = vec![0u64; limbs];
    let point = curve.double_mul(&k, &zero, &curve.g);
    let z_inv = curve.p.invert_prime(&point.z);
    let mut r = curve.p.to_plain(&curve.mul(&point.x, &curve.sq(&z_inv)));
    if bignum::cmp(&r, n.n()) != Ordering::Less {
        bignum::sub_assign(&mut r, n.n());
    }
    // s = k⁻¹ (e + r·d) mod n.
    let rd = n.mont_mul(&n.to_mont(&r), &n.to_mont(&d));
    let sum = n.add(&n.to_mont(&e), &rd);
    let s = n.to_plain(&n.mont_mul(&n.invert_prime(&n.to_mont(&k)), &sum));
    let int = |value: &[u64]| {
        let mut bytes = bignum::to_be_bytes(value, curve.len + 1).unwrap();
        while bytes.len() > 1 && bytes[0] == 0 && bytes[1] & 0x80 == 0 {
            bytes.remove(0);
        }
        [vec![0x02, bytes.len() as u8], bytes].concat()
    };
    let body = [int(&r), int(&s)].concat();
    [vec![0x30, body.len() as u8], body].concat()
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

    #[test]
    fn the_generators_are_on_their_curves() {
        for id in [CurveId::P256, CurveId::P384] {
            let curve = Curve::new(id).unwrap();
            let mut encoded = vec![0x04];
            encoded.extend(bignum::to_be_bytes(&curve.p.to_plain(&curve.g.x), curve.len).unwrap());
            encoded.extend(bignum::to_be_bytes(&curve.p.to_plain(&curve.g.y), curve.len).unwrap());
            curve.decode_point(&encoded).unwrap();
            // y → y + 1 leaves the curve; x = p is out of range.
            let last = encoded.len() - 1;
            encoded[last] ^= 1;
            assert_eq!(curve.decode_point(&encoded).err(), Some(Error));
            let mut p_as_x = vec![0x04];
            p_as_x.extend(bignum::to_be_bytes(curve.p.n(), curve.len).unwrap());
            p_as_x.extend(vec![0u8; curve.len]);
            assert_eq!(curve.decode_point(&p_as_x).err(), Some(Error));
        }
    }

    #[test]
    fn n_times_g_is_infinity_and_doubling_agrees_with_adding() {
        for id in [CurveId::P256, CurveId::P384] {
            let curve = Curve::new(id).unwrap();
            let one = {
                let mut one = vec![0u64; curve.n.limbs()];
                one[0] = 1;
                one
            };
            // (n − 1)·G + 1·G = ∞, through the P = −Q case.
            let mut n_minus_1 = curve.n.n().to_vec();
            bignum::sub_assign(&mut n_minus_1, &one);
            let zero = vec![0u64; curve.n.limbs()];
            let minus_g = curve.double_mul(&n_minus_1, &zero, &curve.g);
            assert!(bignum::is_zero(&curve.add_points(&minus_g, &curve.g).z));
            // 1·G + 1·G goes through the P = Q case and must equal 2·G.
            let two_g = curve.double(&curve.g);
            let sum = curve.double_mul(&one, &one, &curve.g);
            let affine_x = |pt: &Point| {
                curve
                    .p
                    .to_plain(&curve.mul(&pt.x, &curve.sq(&curve.p.invert_prime(&pt.z))))
            };
            assert_eq!(affine_x(&sum), affine_x(&two_g));
        }
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
                let public = key
                    .verifying_key()
                    .to_encoded_point(false)
                    .as_bytes()
                    .to_vec();
                Some((public, sig.to_der().as_bytes().to_vec()))
            }
            CurveId::P384 => {
                let key = p384::ecdsa::SigningKey::from_slice(secret).ok()?;
                let sig: p384::ecdsa::Signature = key.sign_prehash(&prehash).ok()?;
                let public = key
                    .verifying_key()
                    .to_encoded_point(false)
                    .as_bytes()
                    .to_vec();
                Some((public, sig.to_der().as_bytes().to_vec()))
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Cross-check against RustCrypto's `p256`/`p384` (`docs/plan-tls.md` §7.2): their signatures verify, and
        /// a changed message or a mutated signature is refused.
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
