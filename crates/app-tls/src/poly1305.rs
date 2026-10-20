//! The Poly1305 one-time authenticator (RFC 8439 §2.5), with the 130-bit accumulator in three limbs of 44, 44 and 42
//! bits and the products in `u128` (the layout of poly1305-donna-64). Nothing branches on the key, the accumulator
//! or the tag.

use zeroize::Zeroize;

use crate::ct;

/// Bytes in a block and in the tag.
pub(crate) const BLOCK_LEN: usize = 16;

const MASK44: u64 = (1 << 44) - 1;
const MASK42: u64 = (1 << 42) - 1;

fn le64(bytes: &[u8; BLOCK_LEN], at: usize) -> u64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(word)
}

/// A Poly1305 computation under one 32-byte one-time key `r ‖ s`.
pub(crate) struct Poly1305 {
    /// `r`, clamped (RFC 8439 §2.5), in 44/44/42-bit limbs.
    r: [u64; 3],
    /// The accumulator `h`, kept below 2^131 between blocks.
    h: [u64; 3],
    /// `s` as two little-endian 64-bit words.
    s: [u64; 2],
    /// A partial block waiting for more input.
    buffer: [u8; BLOCK_LEN],
    buffered: usize,
}

impl Poly1305 {
    /// Starts a MAC with the one-time key `key` = `r ‖ s` and clamps `r` (RFC 8439 §2.5).
    pub(crate) fn new(key: &[u8; 32]) -> Self {
        let mut r = [0u8; BLOCK_LEN];
        let mut s = [0u8; BLOCK_LEN];
        r.copy_from_slice(&key[..16]);
        s.copy_from_slice(&key[16..]);
        let (t0, t1) = (le64(&r, 0), le64(&r, 8));
        let state = Self {
            // The clamp: the top four bits of bytes 3, 7, 11, 15 and the bottom two bits of bytes 4, 8, 12 cleared.
            r: [
                t0 & 0xffc_0fff_ffff,
                ((t0 >> 44) | (t1 << 20)) & 0xfff_ffc0_ffff,
                (t1 >> 24) & 0x00f_ffff_fc0f,
            ],
            h: [0; 3],
            s: [le64(&s, 0), le64(&s, 8)],
            buffer: [0; BLOCK_LEN],
            buffered: 0,
        };
        r.zeroize();
        s.zeroize();
        state
    }

    /// Adds one 16-byte block to the accumulator and multiplies by `r` mod 2^130 − 5 (RFC 8439 §2.5.1). `hibit` is
    /// 2^128 (as bit 40 of the top limb) for a full block; a final short block carries its 0x01 byte itself.
    fn block(&mut self, m: &[u8; BLOCK_LEN], hibit: u64) {
        let [r0, r1, r2] = self.r;
        // 2^132 ≡ 4·5 (mod 2^130 − 5), so limb products that overflow the top fold back in times 20.
        let (s1, s2) = (r1 * 20, r2 * 20);
        let (t0, t1) = (le64(m, 0), le64(m, 8));
        let h0 = self.h[0] + (t0 & MASK44);
        let h1 = self.h[1] + (((t0 >> 44) | (t1 << 20)) & MASK44);
        let h2 = self.h[2] + (((t1 >> 24) & MASK42) | hibit);

        let wide = |a: u64, b: u64| u128::from(a) * u128::from(b);
        let d0 = wide(h0, r0) + wide(h1, s2) + wide(h2, s1);
        let mut d1 = wide(h0, r1) + wide(h1, r0) + wide(h2, s2);
        let mut d2 = wide(h0, r2) + wide(h1, r1) + wide(h2, r0);

        // Partial reduction: carry through the limbs, the top carry folds back times 5.
        let mut c = d0 >> 44;
        let mut h0 = (d0 as u64) & MASK44;
        d1 += c;
        c = d1 >> 44;
        let h1 = (d1 as u64) & MASK44;
        d2 += c;
        c = d2 >> 42;
        let h2 = (d2 as u64) & MASK42;
        h0 += (c as u64) * 5;
        let carry = h0 >> 44;
        h0 &= MASK44;
        self.h = [h0, h1 + carry, h2];
    }

    /// Feeds `data` into the MAC.
    pub(crate) fn update(&mut self, mut data: &[u8]) {
        if self.buffered > 0 {
            let take = (BLOCK_LEN - self.buffered).min(data.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&data[..take]);
            self.buffered += take;
            data = &data[take..];
            if self.buffered < BLOCK_LEN {
                return;
            }
            let block = self.buffer;
            self.block(&block, 1 << 40);
            self.buffered = 0;
        }
        let (blocks, rest) = data.as_chunks::<BLOCK_LEN>();
        for block in blocks {
            self.block(block, 1 << 40);
        }
        self.buffer[..rest.len()].copy_from_slice(rest);
        self.buffered = rest.len();
    }

    /// Feeds `data`, then zeros up to the next 16-byte boundary (`pad16` of RFC 8439 §2.8).
    pub(crate) fn update_padded(&mut self, data: &[u8]) {
        self.update(data);
        if self.buffered > 0 {
            self.update(&[0u8; BLOCK_LEN][self.buffered..]);
        }
    }

    /// The tag: the last block, the full reduction mod 2^130 − 5 and the addition of `s` mod 2^128, all with masks
    /// instead of branches (RFC 8439 §2.5.1).
    pub(crate) fn finish(mut self) -> [u8; BLOCK_LEN] {
        if self.buffered > 0 {
            let mut last = [0u8; BLOCK_LEN];
            last[..self.buffered].copy_from_slice(&self.buffer[..self.buffered]);
            last[self.buffered] = 1;
            self.block(&last, 0);
            last.zeroize();
        }
        let [mut h0, mut h1, mut h2] = self.h;

        // Carry fully: h < 2^130 + a little.
        let mut c = h1 >> 44;
        h1 &= MASK44;
        h2 += c;
        c = h2 >> 42;
        h2 &= MASK42;
        h0 += c * 5;
        c = h0 >> 44;
        h0 &= MASK44;
        h1 += c;
        c = h1 >> 44;
        h1 &= MASK44;
        h2 += c;
        c = h2 >> 42;
        h2 &= MASK42;
        h0 += c * 5;
        c = h0 >> 44;
        h0 &= MASK44;
        h1 += c;

        // g = h + 5 − 2^130 = h − p. If that doesn't borrow, h ≥ p and the result is g; otherwise it is h.
        let mut g0 = h0 + 5;
        c = g0 >> 44;
        g0 &= MASK44;
        let mut g1 = h1 + c;
        c = g1 >> 44;
        g1 &= MASK44;
        let g2 = (h2 + c).wrapping_sub(1 << 42);
        let take_g = ct::mask(1 ^ (g2 >> 63));
        h0 = ct::select(take_g, g0, h0);
        h1 = ct::select(take_g, g1, h1);
        h2 = ct::select(take_g, g2 & MASK42, h2);

        // h + s mod 2^128.
        let [s0, s1] = self.s;
        h0 += s0 & MASK44;
        c = h0 >> 44;
        h0 &= MASK44;
        h1 += (((s0 >> 44) | (s1 << 20)) & MASK44) + c;
        c = h1 >> 44;
        h1 &= MASK44;
        h2 = (h2 + (s1 >> 24) + c) & MASK42;

        let lo = h0 | (h1 << 44);
        let hi = (h1 >> 20) | (h2 << 24);
        let mut tag = [0u8; BLOCK_LEN];
        tag[..8].copy_from_slice(&lo.to_le_bytes());
        tag[8..].copy_from_slice(&hi.to_le_bytes());
        tag
    }
}

impl Drop for Poly1305 {
    fn drop(&mut self) {
        self.r.zeroize();
        self.h.zeroize();
        self.s.zeroize();
        self.buffer.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{hex, hex_array};

    fn mac(key: &[u8; 32], data: &[u8]) -> [u8; 16] {
        let mut p = Poly1305::new(key);
        p.update(data);
        p.finish()
    }

    #[test]
    fn rfc8439_2_5_2() {
        let key = hex_array("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b");
        let tag = mac(&key, b"Cryptographic Forum Research Group");
        assert_eq!(tag.to_vec(), hex("a8061dc1305136c6c22b8baf0c0127a9"));
    }

    #[test]
    fn rfc8439_appendix_a3() {
        for (n, (key, text, tag)) in A3.iter().enumerate() {
            assert_eq!(
                mac(&hex_array(key), &hex(text)).to_vec(),
                hex(tag),
                "vector {}",
                n + 1
            );
        }
    }

    #[test]
    fn input_split_anywhere_gives_the_same_tag() {
        let key: [u8; 32] = core::array::from_fn(|i| (i as u8).wrapping_mul(37) ^ 0x5a);
        let data: Vec<u8> = (0..200u8).collect();
        let whole = mac(&key, &data);
        for split in 0..data.len() {
            let mut p = Poly1305::new(&key);
            p.update(&data[..split]);
            p.update(&data[split..]);
            assert_eq!(p.finish(), whole, "split at {split}");
        }
    }

    /// RFC 8439 Appendix A.3: (one-time key, text, tag).
    const A3: &[(&str, &str, &str)] = &[
        (
            "0000000000000000000000000000000000000000000000000000000000000000",
            concat!(
                "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                "00000000000000000000000000000000",
            ),
            "00000000000000000000000000000000",
        ),
        (
            "0000000000000000000000000000000036e5f6b5c5e06070f0efca96227a863e",
            concat!(
                "416e79207375626d697373696f6e20746f20746865204945544620696e74656e6465642062792074686520436f6e7472",
                "696275746f7220666f72207075626c69636174696f6e20617320616c6c206f722070617274206f6620616e2049455446",
                "20496e7465726e65742d4472616674206f722052464320616e6420616e792073746174656d656e74206d616465207769",
                "7468696e2074686520636f6e74657874206f6620616e204945544620616374697669747920697320636f6e7369646572",
                "656420616e20224945544620436f6e747269627574696f6e222e20537563682073746174656d656e747320696e636c75",
                "6465206f72616c2073746174656d656e747320696e20494554462073657373696f6e732c2061732077656c6c20617320",
                "7772697474656e20616e6420656c656374726f6e696320636f6d6d756e69636174696f6e73206d61646520617420616e",
                "792074696d65206f7220706c6163652c207768696368206172652061646472657373656420746f",
            ),
            "36e5f6b5c5e06070f0efca96227a863e",
        ),
        (
            "36e5f6b5c5e06070f0efca96227a863e00000000000000000000000000000000",
            concat!(
                "416e79207375626d697373696f6e20746f20746865204945544620696e74656e6465642062792074686520436f6e7472",
                "696275746f7220666f72207075626c69636174696f6e20617320616c6c206f722070617274206f6620616e2049455446",
                "20496e7465726e65742d4472616674206f722052464320616e6420616e792073746174656d656e74206d616465207769",
                "7468696e2074686520636f6e74657874206f6620616e204945544620616374697669747920697320636f6e7369646572",
                "656420616e20224945544620436f6e747269627574696f6e222e20537563682073746174656d656e747320696e636c75",
                "6465206f72616c2073746174656d656e747320696e20494554462073657373696f6e732c2061732077656c6c20617320",
                "7772697474656e20616e6420656c656374726f6e696320636f6d6d756e69636174696f6e73206d61646520617420616e",
                "792074696d65206f7220706c6163652c207768696368206172652061646472657373656420746f",
            ),
            "f3477e7cd95417af89a6b8794c310cf0",
        ),
        (
            "1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0",
            concat!(
                "2754776173206272696c6c69672c20616e642074686520736c6974687920746f7665730a446964206779726520616e64",
                "2067696d626c6520696e2074686520776162653a0a416c6c206d696d737920776572652074686520626f726f676f7665",
                "732c0a416e6420746865206d6f6d65207261746873206f757467726162652e",
            ),
            "4541669a7eaaee61e708dc7cbcc5eb62",
        ),
        (
            "0200000000000000000000000000000000000000000000000000000000000000",
            "ffffffffffffffffffffffffffffffff",
            "03000000000000000000000000000000",
        ),
        (
            "02000000000000000000000000000000ffffffffffffffffffffffffffffffff",
            "02000000000000000000000000000000",
            "03000000000000000000000000000000",
        ),
        (
            "0100000000000000000000000000000000000000000000000000000000000000",
            "fffffffffffffffffffffffffffffffff0ffffffffffffffffffffffffffffff11000000000000000000000000000000",
            "05000000000000000000000000000000",
        ),
        (
            "0100000000000000000000000000000000000000000000000000000000000000",
            "fffffffffffffffffffffffffffffffffbfefefefefefefefefefefefefefefe01010101010101010101010101010101",
            "00000000000000000000000000000000",
        ),
        (
            "0200000000000000000000000000000000000000000000000000000000000000",
            "fdffffffffffffffffffffffffffffff",
            "faffffffffffffffffffffffffffffff",
        ),
        (
            "0100000000000000040000000000000000000000000000000000000000000000",
            concat!(
                "e33594d7505e43b900000000000000003394d7505e4379cd010000000000000000000000000000000000000000000000",
                "01000000000000000000000000000000",
            ),
            "14000000000000005500000000000000",
        ),
        (
            "0100000000000000040000000000000000000000000000000000000000000000",
            "e33594d7505e43b900000000000000003394d7505e4379cd010000000000000000000000000000000000000000000000",
            "13000000000000000000000000000000",
        ),
    ];
}
