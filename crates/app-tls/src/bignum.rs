//! Unsigned big integers for signature verification: little-endian `u64` limbs, comparison, addition, subtraction,
//! and Montgomery multiplication (CIOS, Koç–Acar–Kaliski 1996) and exponentiation modulo an odd modulus. Every input
//! is public (keys, signatures, hashes), so the code may take variable time (`docs/plan-tls.md` §4.5, rule 2).

use core::cmp::Ordering;

use crate::Error;

/// Little-endian limbs, always exactly as many as the modulus they belong to.
pub(crate) type Limbs = Vec<u64>;

/// Reads a big-endian byte string into `len` limbs. Refuses a value that doesn't fit.
pub(crate) fn from_be_bytes(bytes: &[u8], len: usize) -> Result<Limbs, Error> {
    let mut limbs = vec![0u64; len];
    for (i, &byte) in bytes.iter().rev().enumerate() {
        let (limb, shift) = (i / 8, 8 * (i % 8));
        match limbs.get_mut(limb) {
            Some(slot) => *slot |= u64::from(byte) << shift,
            None if byte == 0 => {}
            None => return Err(Error),
        }
    }
    Ok(limbs)
}

/// Writes `a` big-endian into exactly `len` bytes. Refuses a value that doesn't fit.
pub(crate) fn to_be_bytes(a: &[u64], len: usize) -> Result<Vec<u8>, Error> {
    let mut out = vec![0u8; len];
    for (i, limb) in a.iter().enumerate() {
        for (j, byte) in limb.to_le_bytes().into_iter().enumerate() {
            let at = 8 * i + j;
            if at < len {
                out[len - 1 - at] = byte;
            } else if byte != 0 {
                return Err(Error);
            }
        }
    }
    Ok(out)
}

/// Compares two numbers of the same limb count.
pub(crate) fn cmp(a: &[u64], b: &[u64]) -> Ordering {
    a.iter().rev().cmp(b.iter().rev())
}

/// Whether `a` is zero.
pub(crate) fn is_zero(a: &[u64]) -> bool {
    a.iter().all(|&limb| limb == 0)
}

/// The number of significant bits of `a`.
pub(crate) fn bits(a: &[u64]) -> usize {
    match a.iter().rposition(|&limb| limb != 0) {
        Some(top) => 64 * top + (64 - a[top].leading_zeros() as usize),
        None => 0,
    }
}

/// `a += b`; returns the carry out of the top limb.
pub(crate) fn add_assign(a: &mut [u64], b: &[u64]) -> bool {
    let mut carry = false;
    for (x, &y) in a.iter_mut().zip(b) {
        let (sum, c1) = x.overflowing_add(y);
        let (sum, c2) = sum.overflowing_add(u64::from(carry));
        *x = sum;
        carry = c1 || c2;
    }
    carry
}

/// `a -= b`; returns the borrow out of the top limb.
pub(crate) fn sub_assign(a: &mut [u64], b: &[u64]) -> bool {
    let mut borrow = false;
    for (x, &y) in a.iter_mut().zip(b) {
        let (diff, b1) = x.overflowing_sub(y);
        let (diff, b2) = diff.overflowing_sub(u64::from(borrow));
        *x = diff;
        borrow = b1 || b2;
    }
    borrow
}

/// An odd modulus `n` > 1 with its Montgomery constants for R = 2^(64·limbs).
#[derive(Clone)]
pub(crate) struct Modulus {
    n: Limbs,
    /// −n⁻¹ mod 2^64.
    n0_inv: u64,
    /// R² mod n.
    rr: Limbs,
}

impl Modulus {
    /// Builds the constants for the odd modulus `n` (big-endian, no leading zero bytes needed).
    pub(crate) fn new(n_be: &[u8]) -> Result<Self, Error> {
        let len = n_be.len().div_ceil(8).max(1);
        let n = from_be_bytes(n_be, len)?;
        if n[0] & 1 == 0 || bits(&n) < 2 {
            return Err(Error);
        }
        // Newton's iteration doubles the correct low bits each round: 1 → 2 → … → 64 after six rounds.
        let mut inv = 1u64;
        for _ in 0..6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(n[0].wrapping_mul(inv)));
        }
        // R² mod n by doubling 1 a total of 2·64·len times, reducing as it goes.
        let mut rr = vec![0u64; len];
        rr[0] = 1;
        for _ in 0..2 * 64 * len {
            let mut carry = 0;
            for limb in rr.iter_mut() {
                let next = *limb >> 63;
                *limb = (*limb << 1) | carry;
                carry = next;
            }
            if carry == 1 || cmp(&rr, &n) != Ordering::Less {
                sub_assign(&mut rr, &n);
            }
        }
        Ok(Self {
            n,
            n0_inv: inv.wrapping_neg(),
            rr,
        })
    }

    /// The modulus.
    pub(crate) fn n(&self) -> &[u64] {
        &self.n
    }

    /// The limb count of every number modulo `n`.
    pub(crate) fn limbs(&self) -> usize {
        self.n.len()
    }

    /// The bit length of `n`.
    pub(crate) fn bits(&self) -> usize {
        bits(&self.n)
    }

    /// `a · b · R⁻¹ mod n` for `a`, `b` < n (CIOS).
    pub(crate) fn mont_mul(&self, a: &[u64], b: &[u64]) -> Limbs {
        let k = self.n.len();
        let mut t = vec![0u64; k + 2];
        for &bi in b {
            let mut carry = 0u128;
            for j in 0..k {
                let sum = u128::from(t[j]) + u128::from(a[j]) * u128::from(bi) + carry;
                t[j] = sum as u64;
                carry = sum >> 64;
            }
            let sum = u128::from(t[k]) + carry;
            t[k] = sum as u64;
            t[k + 1] = (sum >> 64) as u64;

            let m = t[0].wrapping_mul(self.n0_inv);
            let mut carry = (u128::from(t[0]) + u128::from(m) * u128::from(self.n[0])) >> 64;
            for j in 1..k {
                let sum = u128::from(t[j]) + u128::from(m) * u128::from(self.n[j]) + carry;
                t[j - 1] = sum as u64;
                carry = sum >> 64;
            }
            let sum = u128::from(t[k]) + carry;
            t[k - 1] = sum as u64;
            t[k] = t[k + 1] + (sum >> 64) as u64;
        }
        let overflow = t[k] != 0;
        t.truncate(k);
        if overflow || cmp(&t, &self.n) != Ordering::Less {
            sub_assign(&mut t, &self.n);
        }
        t
    }

    /// `a` in Montgomery form, `a · R mod n`, for `a` < n.
    pub(crate) fn to_mont(&self, a: &[u64]) -> Limbs {
        self.mont_mul(a, &self.rr)
    }

    /// `a` out of Montgomery form back to a plain integer, `a · R⁻¹ mod n`.
    pub(crate) fn to_plain(&self, a: &[u64]) -> Limbs {
        let mut one = vec![0u64; self.n.len()];
        one[0] = 1;
        self.mont_mul(a, &one)
    }

    /// 1 in Montgomery form.
    pub(crate) fn one(&self) -> Limbs {
        let mut one = vec![0u64; self.n.len()];
        one[0] = 1;
        self.to_mont(&one)
    }

    /// `a + b mod n` for `a`, `b` < n.
    pub(crate) fn add(&self, a: &[u64], b: &[u64]) -> Limbs {
        let mut sum = a.to_vec();
        let carry = add_assign(&mut sum, b);
        if carry || cmp(&sum, &self.n) != Ordering::Less {
            sub_assign(&mut sum, &self.n);
        }
        sum
    }

    /// `a − b mod n` for `a`, `b` < n.
    pub(crate) fn sub(&self, a: &[u64], b: &[u64]) -> Limbs {
        let mut diff = a.to_vec();
        if sub_assign(&mut diff, b) {
            add_assign(&mut diff, &self.n);
        }
        diff
    }

    /// `base^exp` with `base` in Montgomery form and the result in Montgomery form. `exp` is big-endian, left to
    /// right, square and multiply.
    pub(crate) fn pow(&self, base: &[u64], exp_be: &[u8]) -> Limbs {
        let mut acc = self.one();
        for byte in exp_be {
            for bit in (0..8).rev() {
                acc = self.mont_mul(&acc, &acc);
                if (byte >> bit) & 1 == 1 {
                    acc = self.mont_mul(&acc, base);
                }
            }
        }
        acc
    }

    /// `a⁻¹ mod n` by Fermat, `a^(n − 2)`, for a prime `n`; `a` and the result in Montgomery form. 0 maps to 0.
    pub(crate) fn invert_prime(&self, a: &[u64]) -> Limbs {
        let mut exp = self.n.clone();
        let mut two = vec![0u64; exp.len()];
        two[0] = 2;
        sub_assign(&mut exp, &two);
        let bytes = self.n.len() * 8;
        match to_be_bytes(&exp, bytes) {
            Ok(exp_be) => self.pow(a, &exp_be),
            // Unreachable: `exp` < n fits in n's limbs.
            Err(Error) => vec![0u64; self.n.len()],
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    /// Reference arithmetic on `u128` for moduli below 2^64.
    fn mod_pow(base: u128, mut exp: u128, n: u128) -> u128 {
        let (mut acc, mut b) = (1u128, base % n);
        while exp > 0 {
            if exp & 1 == 1 {
                acc = acc * b % n;
            }
            b = b * b % n;
            exp >>= 1;
        }
        acc
    }

    #[test]
    fn bytes_round_trip() {
        let limbs = from_be_bytes(&[1, 2, 3, 4, 5, 6, 7, 8, 9], 2).unwrap();
        assert_eq!(limbs, vec![0x0203_0405_0607_0809, 1]);
        assert_eq!(
            to_be_bytes(&limbs, 9).unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
        assert_eq!(
            to_be_bytes(&limbs, 11).unwrap(),
            vec![0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
        assert_eq!(to_be_bytes(&limbs, 8), Err(Error));
        assert_eq!(from_be_bytes(&[0, 0, 1], 1).unwrap(), vec![1]);
        assert_eq!(from_be_bytes(&[1; 9], 1), Err(Error));
        assert_eq!(bits(&[0, 1]), 65);
        assert_eq!(bits(&[0, 0]), 0);
        assert!(is_zero(&[0, 0]) && !is_zero(&[0, 1]));
    }

    #[test]
    fn refuses_even_or_trivial_moduli() {
        assert!(Modulus::new(&[10]).is_err());
        assert!(Modulus::new(&[1]).is_err());
        assert!(Modulus::new(&[]).is_err());
        assert!(Modulus::new(&[3]).is_ok());
    }

    #[test]
    fn inverts_mod_a_prime() {
        // p = 2^127 − 1.
        let p = Modulus::new(&[
            0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff,
        ])
        .unwrap();
        let a = p.to_mont(&[12345, 678]);
        let inv = p.invert_prime(&a);
        assert_eq!(p.to_plain(&p.mont_mul(&a, &inv)), vec![1, 0]);
    }

    proptest! {
        /// Montgomery arithmetic on one limb agrees with `u128` arithmetic.
        #[test]
        fn one_limb_matches_u128(n in any::<u64>(), a in any::<u64>(), b in any::<u64>(), e in any::<u32>()) {
            let n = (n | 1).max(3);
            let (a, b) = (a % n, b % n);
            let m = Modulus::new(&n.to_be_bytes()).unwrap();
            let (am, bm) = (m.to_mont(&[a]), m.to_mont(&[b]));
            let (n, a128, b128) = (u128::from(n), u128::from(a), u128::from(b));
            prop_assert_eq!(m.to_plain(&m.mont_mul(&am, &bm)), vec![(a128 * b128 % n) as u64]);
            prop_assert_eq!(m.to_plain(&m.add(&am, &bm)), vec![((a128 + b128) % n) as u64]);
            prop_assert_eq!(m.to_plain(&m.sub(&am, &bm)), vec![((a128 + n - b128) % n) as u64]);
            prop_assert_eq!(m.to_plain(&m.pow(&am, &e.to_be_bytes())), vec![mod_pow(a128, u128::from(e), n) as u64]);
        }

        /// On two limbs, `(a·b)·c = a·(b·c)` and `a·1 = a`, with carries across limbs.
        #[test]
        fn two_limbs_associate(n in any::<[u64; 2]>(), a in any::<[u64; 2]>(), b in any::<[u64; 2]>(), c in any::<[u64; 2]>()) {
            let n = [n[0] | 1, n[1] | (1 << 63)];
            let m = Modulus::new(&to_be_bytes(&n, 16).unwrap()).unwrap();
            let reduce = |x: [u64; 2]| { let mut x = x.to_vec(); while cmp(&x, &n) != Ordering::Less { sub_assign(&mut x, &n); } x };
            let (a, b, c) = (m.to_mont(&reduce(a)), m.to_mont(&reduce(b)), m.to_mont(&reduce(c)));
            prop_assert_eq!(m.mont_mul(&m.mont_mul(&a, &b), &c), m.mont_mul(&a, &m.mont_mul(&b, &c)));
            prop_assert_eq!(m.mont_mul(&a, &m.one()), a.clone());
            prop_assert_eq!(m.sub(&m.add(&a, &b), &b), a);
        }
    }
}
