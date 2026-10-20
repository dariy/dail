//! The ChaCha20 stream cipher (RFC 8439 §2.1–2.4): 256-bit key, 96-bit nonce, 32-bit block counter.

use zeroize::Zeroize;

use crate::Error;

/// "expand 32-byte k" as four little-endian words (RFC 8439 §2.3).
const CONSTANTS: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

/// Bytes of keystream in one block.
pub(crate) const BLOCK_LEN: usize = 64;

/// The quarter round on words `a`, `b`, `c`, `d` of the state (RFC 8439 §2.1, §2.2).
#[inline(always)]
fn quarter_round(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(7);
}

/// The initial state: constants, key, block counter and nonce, all little-endian (RFC 8439 §2.3).
fn initial_state(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u32; 16] {
    let mut s = [0u32; 16];
    s[..4].copy_from_slice(&CONSTANTS);
    for (word, bytes) in s[4..12].iter_mut().zip(key.chunks_exact(4)) {
        *word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    s[12] = counter;
    for (word, bytes) in s[13..].iter_mut().zip(nonce.chunks_exact(4)) {
        *word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    s
}

/// The ChaCha20 block function (RFC 8439 §2.3): 20 rounds over the initial state, the initial state added back, the
/// result serialized little-endian.
pub(crate) fn block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; BLOCK_LEN] {
    let mut initial = initial_state(key, counter, nonce);
    let mut s = initial;
    for _ in 0..10 {
        quarter_round(&mut s, 0, 4, 8, 12);
        quarter_round(&mut s, 1, 5, 9, 13);
        quarter_round(&mut s, 2, 6, 10, 14);
        quarter_round(&mut s, 3, 7, 11, 15);
        quarter_round(&mut s, 0, 5, 10, 15);
        quarter_round(&mut s, 1, 6, 11, 12);
        quarter_round(&mut s, 2, 7, 8, 13);
        quarter_round(&mut s, 3, 4, 9, 14);
    }
    let mut out = [0u8; BLOCK_LEN];
    for ((bytes, word), init) in out.chunks_exact_mut(4).zip(&s).zip(&initial) {
        bytes.copy_from_slice(&word.wrapping_add(*init).to_le_bytes());
    }
    s.zeroize();
    initial.zeroize();
    out
}

/// Whether `len` bytes of keystream starting at block `counter` fit before the 32-bit counter would wrap.
pub(crate) fn fits(counter: u32, len: usize) -> bool {
    let blocks = (len as u64).div_ceil(BLOCK_LEN as u64);
    u64::from(counter) + blocks <= 1 << 32
}

/// XORs the keystream that starts at block `counter` into `data` (RFC 8439 §2.4). Refuses data that would make the
/// block counter wrap, so a keystream block is never used twice.
pub(crate) fn apply_keystream(
    key: &[u8; 32],
    nonce: &[u8; 12],
    counter: u32,
    data: &mut [u8],
) -> Result<(), Error> {
    if !fits(counter, data.len()) {
        return Err(Error);
    }
    let mut n = counter;
    for chunk in data.chunks_mut(BLOCK_LEN) {
        let mut stream = block(key, n, nonce);
        for (byte, k) in chunk.iter_mut().zip(&stream) {
            *byte ^= k;
        }
        stream.zeroize();
        // Wraps only after the last block, when `fits` allowed exactly 2^32 blocks.
        n = n.wrapping_add(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{hex, hex_array};

    #[test]
    fn rfc8439_2_1_1_quarter_round() {
        let mut s = [0u32; 16];
        s[..4].copy_from_slice(&[0x1111_1111, 0x0102_0304, 0x9b8d_6f43, 0x0123_4567]);
        quarter_round(&mut s, 0, 1, 2, 3);
        assert_eq!(s[..4], [0xea2a_92f4, 0xcb1c_f8ce, 0x4581_472e, 0x5881_c4bb]);
    }

    #[test]
    fn rfc8439_2_2_1_quarter_round_on_state() {
        let mut s = [
            0x8795_31e0,
            0xc5ec_f37d,
            0x5164_61b1,
            0xc9a6_2f8a,
            0x44c2_0ef3,
            0x3390_af7f,
            0xd9fc_690b,
            0x2a5f_714c,
            0x5337_2767,
            0xb00a_5631,
            0x974c_541a,
            0x359e_9963,
            0x5c97_1061,
            0x3d63_1689,
            0x2098_d9d6,
            0x91db_d320,
        ];
        quarter_round(&mut s, 2, 7, 8, 13);
        assert_eq!(
            s,
            [
                0x8795_31e0,
                0xc5ec_f37d,
                0xbdb8_86dc,
                0xc9a6_2f8a,
                0x44c2_0ef3,
                0x3390_af7f,
                0xd9fc_690b,
                0xcfac_afd2,
                0xe46b_ea80,
                0xb00a_5631,
                0x974c_541a,
                0x359e_9963,
                0x5c97_1061,
                0xccc0_7c79,
                0x2098_d9d6,
                0x91db_d320,
            ]
        );
    }

    const KEY_00_1F: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    #[test]
    fn rfc8439_2_3_2_block() {
        let out = block(
            &hex_array(KEY_00_1F),
            1,
            &hex_array("000000090000004a00000000"),
        );
        let expected = concat!(
            "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c068030422aa9ac3d46c4e",
            "d2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e",
        );
        assert_eq!(out.to_vec(), hex(expected));
    }

    #[test]
    fn rfc8439_2_4_2_encryption() {
        let mut data = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.".to_vec();
        apply_keystream(
            &hex_array(KEY_00_1F),
            &hex_array("000000000000004a00000000"),
            1,
            &mut data,
        )
        .unwrap();
        let expected = concat!(
            "6e2e359a2568f98041ba0728dd0d6981e97e7aec1d4360c20a27afccfd9fae0b",
            "f91b65c5524733ab8f593dabcd62b3571639d624e65152ab8f530c359f0861d8",
            "07ca0dbf500d6a6156a38e088a22b65e52bc514d16ccf806818ce91ab7793736",
            "5af90bbf74a35be6b40b8eedf2785e42874d",
        );
        assert_eq!(data, hex(expected));
    }

    #[test]
    fn rfc8439_appendix_a1_blocks() {
        for (key, nonce, counter, keystream) in A1 {
            let out = block(&hex_array(key), *counter, &hex_array(nonce));
            assert_eq!(out.to_vec(), hex(keystream));
        }
    }

    #[test]
    fn rfc8439_appendix_a2_encryption() {
        for (key, nonce, counter, plaintext, ciphertext) in A2 {
            let mut data = hex(plaintext);
            apply_keystream(&hex_array(key), &hex_array(nonce), *counter, &mut data).unwrap();
            assert_eq!(data, hex(ciphertext));
        }
    }

    #[test]
    fn the_counter_never_wraps() {
        let (key, nonce) = ([0u8; 32], [0u8; 12]);
        assert!(fits(u32::MAX, 64));
        assert!(!fits(u32::MAX, 65));
        assert!(fits(0, 0));
        assert_eq!(
            apply_keystream(&key, &nonce, u32::MAX, &mut [0u8; 65]),
            Err(Error)
        );
        apply_keystream(&key, &nonce, u32::MAX, &mut [0u8; 64]).unwrap();
    }

    /// RFC 8439 Appendix A.1: (key, nonce, counter, keystream).
    const A1: &[(&str, &str, u32, &str)] = &[
        (
            "0000000000000000000000000000000000000000000000000000000000000000",
            "000000000000000000000000",
            0,
            concat!(
                "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7da41597c5157488d7724e03fb8d84a37",
                "6a43b8f41518a11cc387b669b2ee6586",
            ),
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000000000",
            "000000000000000000000000",
            1,
            concat!(
                "9f07e7be5551387a98ba977c732d080dcb0f29a048e3656912c6533e32ee7aed29b721769ce64e43d57133b074d839d5",
                "31ed1f28510afb45ace10a1f4b794d6f",
            ),
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000000001",
            "000000000000000000000000",
            1,
            concat!(
                "3aeb5224ecf849929b9d828db1ced4dd832025e8018b8160b82284f3c949aa5a8eca00bbb4a73bdad192b5c42f73f2fd",
                "4e273644c8b36125a64addeb006c13a0",
            ),
        ),
        (
            "00ff000000000000000000000000000000000000000000000000000000000000",
            "000000000000000000000000",
            2,
            concat!(
                "72d54dfbf12ec44b362692df94137f328fea8da73990265ec1bbbea1ae9af0ca13b25aa26cb4a648cb9b9d1be65b2c09",
                "24a66c54d545ec1b7374f4872e99f096",
            ),
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000000000",
            "000000000000000000000002",
            0,
            concat!(
                "c2c64d378cd536374ae204b9ef933fcd1a8b2288b3dfa49672ab765b54ee27c78a970e0e955c14f3a88e741b97c286f7",
                "5f8fc299e8148362fa198a39531bed6d",
            ),
        ),
    ];

    /// RFC 8439 Appendix A.2: (key, nonce, counter, plaintext, ciphertext).
    const A2: &[(&str, &str, u32, &str, &str)] = &[
        (
            "0000000000000000000000000000000000000000000000000000000000000000",
            "000000000000000000000000",
            0,
            concat!(
                "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                "00000000000000000000000000000000",
            ),
            concat!(
                "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7da41597c5157488d7724e03fb8d84a37",
                "6a43b8f41518a11cc387b669b2ee6586",
            ),
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000000001",
            "000000000000000000000002",
            1,
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
            concat!(
                "a3fbf07df3fa2fde4f376ca23e82737041605d9f4f4f57bd8cff2c1d4b7955ec2a97948bd3722915c8f3d337f7d37005",
                "0e9e96d647b7c39f56e031ca5eb6250d4042e02785ececfa4b4bb5e8ead0440e20b6e8db09d881a7c6132f420e527950",
                "42bdfa7773d8a9051447b3291ce1411c680465552aa6c405b7764d5e87bea85ad00f8449ed8f72d0d662ab052691ca66",
                "424bc86d2df80ea41f43abf937d3259dc4b2d0dfb48a6c9139ddd7f76966e928e635553ba76c5c879d7b35d49eb2e62b",
                "0871cdac638939e25e8a1e0ef9d5280fa8ca328b351c3c765989cbcf3daa8b6ccc3aaf9f3979c92b3720fc88dc95ed84",
                "a1be059c6499b9fda236e7e818b04b0bc39c1e876b193bfe5569753f88128cc08aaa9b63d1a16f80ef2554d7189c411f",
                "5869ca52c5b83fa36ff216b9c1d30062bebcfd2dc5bce0911934fda79a86f6e698ced759c3ff9b6477338f3da4f9cd85",
                "14ea9982ccafb341b2384dd902f3d1ab7ac61dd29c6f21ba5b862f3730e37cfdc4fd806c22f221",
            ),
        ),
        (
            "1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0",
            "000000000000000000000002",
            42,
            concat!(
                "2754776173206272696c6c69672c20616e642074686520736c6974687920746f7665730a446964206779726520616e64",
                "2067696d626c6520696e2074686520776162653a0a416c6c206d696d737920776572652074686520626f726f676f7665",
                "732c0a416e6420746865206d6f6d65207261746873206f757467726162652e",
            ),
            concat!(
                "62e6347f95ed87a45ffae7426f27a1df5fb69110044c0d73118effa95b01e5cf166d3df2d721caf9b21e5fb14c616871",
                "fd84c54f9d65b283196c7fe4f60553ebf39c6402c42234e32a356b3e764312a61a5532055716ead6962568f87d3f3f77",
                "04c6a8d1bcd1bf4d50d6154b6da731b187b58dfd728afa36757a797ac188d1",
            ),
        ),
    ];
}
