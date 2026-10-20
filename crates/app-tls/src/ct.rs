//! Constant-time helpers for secret data (`docs/plan-tls.md` §4.5): masks, select, swap and equality, all without
//! branches on the values.

use core::hint::black_box;

/// All ones when `bit` is 1, zero when it is 0. `bit` must be 0 or 1. `black_box` keeps the compiler from turning
/// the mask back into a branch.
#[inline]
pub(crate) fn mask(bit: u64) -> u64 {
    black_box(0u64.wrapping_sub(bit & 1))
}

/// `a` where `mask` is all ones, `b` where it is zero.
#[inline]
pub(crate) fn select(mask: u64, a: u64, b: u64) -> u64 {
    b ^ (mask & (a ^ b))
}

/// Swaps `a` and `b` limb by limb when `mask` is all ones; leaves them when it is zero (RFC 7748 §5, `cswap`).
#[inline]
pub(crate) fn swap<const N: usize>(mask: u64, a: &mut [u64; N], b: &mut [u64; N]) {
    for (x, y) in a.iter_mut().zip(b.iter_mut()) {
        let t = mask & (*x ^ *y);
        *x ^= t;
        *y ^= t;
    }
}

/// Whether `a` and `b` are equal. The time depends only on the lengths, which are public: every byte pair is
/// compared and the XORs are OR-accumulated.
pub(crate) fn eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let diff = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    black_box(diff) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_select_swap() {
        assert_eq!(mask(0), 0);
        assert_eq!(mask(1), u64::MAX);
        assert_eq!(select(mask(1), 7, 9), 7);
        assert_eq!(select(mask(0), 7, 9), 9);
        let (mut a, mut b) = ([1, 2], [3, 4]);
        swap(mask(0), &mut a, &mut b);
        assert_eq!((a, b), ([1, 2], [3, 4]));
        swap(mask(1), &mut a, &mut b);
        assert_eq!((a, b), ([3, 4], [1, 2]));
    }

    #[test]
    fn equality() {
        assert!(eq(b"abc", b"abc"));
        assert!(!eq(b"abc", b"abd"));
        assert!(!eq(b"abc", b"ab"));
        assert!(eq(b"", b""));
    }
}
