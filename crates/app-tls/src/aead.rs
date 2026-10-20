//! The ChaCha20-Poly1305 AEAD (RFC 8439 §2.8).

use rustls::crypto::cipher::{
    AeadKey, InboundOpaqueMessage, InboundPlainMessage, Iv, KeyBlockShape, MessageDecrypter,
    MessageEncrypter, Nonce, OutboundOpaqueMessage, OutboundPlainMessage, PrefixedPayload,
    Tls12AeadAlgorithm, Tls13AeadAlgorithm, UnsupportedOperationError, make_tls12_aad,
    make_tls13_aad,
};
use rustls::{ConnectionTrafficSecrets, ContentType, ProtocolVersion};
use zeroize::Zeroize;

use crate::poly1305::Poly1305;
use crate::{Error, chacha20, ct};

/// Key length in bytes.
pub(crate) const KEY_LEN: usize = 32;
/// Nonce length in bytes.
pub(crate) const NONCE_LEN: usize = 12;
/// Tag length in bytes.
pub(crate) const TAG_LEN: usize = 16;

/// The Poly1305 one-time key: the first 32 bytes of ChaCha20 block 0 (RFC 8439 §2.6).
fn poly1305_key(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN]) -> [u8; 32] {
    let mut block = chacha20::block(key, 0, nonce);
    let mut one_time = [0u8; 32];
    one_time.copy_from_slice(&block[..32]);
    block.zeroize();
    one_time
}

/// The tag over `aad` and `ciphertext`: `AAD ‖ pad16 ‖ ciphertext ‖ pad16 ‖ le64(len(AAD)) ‖ le64(len(ciphertext))`
/// (RFC 8439 §2.8).
fn tag(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    ciphertext: &[u8],
) -> [u8; TAG_LEN] {
    let mut one_time = poly1305_key(key, nonce);
    let mut mac = Poly1305::new(&one_time);
    one_time.zeroize();
    mac.update_padded(aad);
    mac.update_padded(ciphertext);
    mac.update(&(aad.len() as u64).to_le_bytes());
    mac.update(&(ciphertext.len() as u64).to_le_bytes());
    mac.finish()
}

/// Encrypts `in_out` in place and returns the tag (RFC 8439 §2.8). The keystream starts at block 1.
pub(crate) fn seal(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    in_out: &mut [u8],
) -> Result<[u8; TAG_LEN], Error> {
    chacha20::apply_keystream(key, nonce, 1, in_out)?;
    Ok(tag(key, nonce, aad, in_out))
}

/// Checks `received` over `aad` and the ciphertext `in_out`, then decrypts `in_out` in place (RFC 8439 §2.8). The
/// tag is compared in constant time before any byte is decrypted; on a mismatch `in_out` is left as it was.
pub(crate) fn open(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    in_out: &mut [u8],
    received: &[u8; TAG_LEN],
) -> Result<(), Error> {
    if !chacha20::fits(1, in_out.len()) {
        return Err(Error);
    }
    let mut expected = tag(key, nonce, aad, in_out);
    let matches = ct::eq(&expected, received);
    expected.zeroize();
    if !matches {
        return Err(Error);
    }
    chacha20::apply_keystream(key, nonce, 1, in_out)
}

/// The largest plaintext in a TLS record (RFC 8446 §5.1, RFC 5246 §6.2.1).
const MAX_FRAGMENT_LEN: usize = 16_384;

/// ChaCha20-Poly1305 as the record protection of TLS 1.3 (RFC 8446 §5.2, §5.3) and TLS 1.2 (RFC 7905).
pub(crate) struct ChaCha20Poly1305;

/// A record key with the nonce base it is used with. The key is wiped on drop. A key of the wrong length can't
/// reach here through rustls (it asks for `key_len` bytes); if one did, the key stays `None` and every record fails
/// instead of panicking.
struct RecordKey {
    key: Option<[u8; KEY_LEN]>,
    iv: Option<Iv>,
}

impl RecordKey {
    fn new(key: &AeadKey, iv: Option<Iv>) -> Self {
        Self {
            key: key.as_ref().try_into().ok(),
            iv,
        }
    }

    /// The key and the record nonce: the IV XOR the sequence number (RFC 8446 §5.3, RFC 7905 §2).
    fn for_record(&self, seq: u64) -> Option<(&[u8; KEY_LEN], [u8; NONCE_LEN])> {
        Some((self.key.as_ref()?, Nonce::new(self.iv.as_ref()?, seq).0))
    }
}

impl Drop for RecordKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl Tls13AeadAlgorithm for ChaCha20Poly1305 {
    fn encrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageEncrypter> {
        Box::new(Tls13Encrypter(RecordKey::new(&key, Some(iv))))
    }

    fn decrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageDecrypter> {
        Box::new(Tls13Decrypter(RecordKey::new(&key, Some(iv))))
    }

    fn key_len(&self) -> usize {
        KEY_LEN
    }

    /// Key extraction (kTLS) is out of scope (`docs/plan-tls.md` §3).
    fn extract_keys(
        &self,
        _key: AeadKey,
        _iv: Iv,
    ) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError> {
        Err(UnsupportedOperationError)
    }
}

impl Tls12AeadAlgorithm for ChaCha20Poly1305 {
    fn encrypter(&self, key: AeadKey, iv: &[u8], _explicit: &[u8]) -> Box<dyn MessageEncrypter> {
        Box::new(Tls12Encrypter(RecordKey::new(&key, tls12_iv(iv))))
    }

    fn decrypter(&self, key: AeadKey, iv: &[u8]) -> Box<dyn MessageDecrypter> {
        Box::new(Tls12Decrypter(RecordKey::new(&key, tls12_iv(iv))))
    }

    /// RFC 7905 §2: a 32-byte key and a 12-byte fixed IV, no explicit nonce.
    fn key_block_shape(&self) -> KeyBlockShape {
        KeyBlockShape {
            enc_key_len: KEY_LEN,
            fixed_iv_len: NONCE_LEN,
            explicit_nonce_len: 0,
        }
    }

    /// Key extraction (kTLS) is out of scope (`docs/plan-tls.md` §3).
    fn extract_keys(
        &self,
        _key: AeadKey,
        _iv: &[u8],
        _explicit: &[u8],
    ) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError> {
        Err(UnsupportedOperationError)
    }
}

fn tls12_iv(iv: &[u8]) -> Option<Iv> {
    <[u8; NONCE_LEN]>::try_from(iv).ok().map(Iv::new)
}

/// Seals the plaintext already in `payload` and appends the tag.
fn seal_payload(
    key: &RecordKey,
    seq: u64,
    aad: &[u8],
    payload: &mut PrefixedPayload,
) -> Result<(), rustls::Error> {
    let (key, nonce) = key.for_record(seq).ok_or(rustls::Error::EncryptError)?;
    let tag =
        seal(key, &nonce, aad, payload.as_mut()).map_err(|Error| rustls::Error::EncryptError)?;
    payload.extend_from_slice(&tag);
    Ok(())
}

/// Checks the tag at the end of `payload` and decrypts the rest in place; returns the plaintext length.
fn open_payload(
    key: &RecordKey,
    seq: u64,
    aad: &[u8],
    payload: &mut [u8],
) -> Result<usize, rustls::Error> {
    let plain_len = payload
        .len()
        .checked_sub(TAG_LEN)
        .ok_or(rustls::Error::DecryptError)?;
    let (key, nonce) = key.for_record(seq).ok_or(rustls::Error::DecryptError)?;
    let (ciphertext, tag) = payload.split_at_mut(plain_len);
    let tag: &[u8; TAG_LEN] = (&*tag)
        .try_into()
        .map_err(|_| rustls::Error::DecryptError)?;
    open(key, &nonce, aad, ciphertext, tag).map_err(|Error| rustls::Error::DecryptError)?;
    Ok(plain_len)
}

struct Tls13Encrypter(RecordKey);
struct Tls13Decrypter(RecordKey);
struct Tls12Encrypter(RecordKey);
struct Tls12Decrypter(RecordKey);

impl MessageEncrypter for Tls13Encrypter {
    /// RFC 8446 §5.2: the plaintext, then the real content type, sealed as an `application_data` record with the
    /// record header as the additional data.
    fn encrypt(
        &mut self,
        msg: OutboundPlainMessage<'_>,
        seq: u64,
    ) -> Result<OutboundOpaqueMessage, rustls::Error> {
        let total_len = self.encrypted_payload_len(msg.payload.len());
        let mut payload = PrefixedPayload::with_capacity(total_len);
        payload.extend_from_chunks(&msg.payload);
        payload.extend_from_slice(&msg.typ.to_array());
        seal_payload(&self.0, seq, &make_tls13_aad(total_len), &mut payload)?;
        // All TLS 1.3 records carry the legacy version 0x0303 (RFC 8446 §5.1).
        Ok(OutboundOpaqueMessage::new(
            ContentType::ApplicationData,
            ProtocolVersion::TLSv1_2,
            payload,
        ))
    }

    fn encrypted_payload_len(&self, payload_len: usize) -> usize {
        payload_len + 1 + TAG_LEN
    }
}

impl MessageDecrypter for Tls13Decrypter {
    fn decrypt<'a>(
        &mut self,
        mut msg: InboundOpaqueMessage<'a>,
        seq: u64,
    ) -> Result<InboundPlainMessage<'a>, rustls::Error> {
        let aad = make_tls13_aad(msg.payload.len());
        let plain_len = open_payload(&self.0, seq, &aad, &mut msg.payload)?;
        msg.payload.truncate(plain_len);
        msg.into_tls13_unpadded_message()
    }
}

impl MessageEncrypter for Tls12Encrypter {
    /// RFC 7905 §2 and RFC 5246 §6.2.3.3: the additional data is the sequence number and the record header.
    fn encrypt(
        &mut self,
        msg: OutboundPlainMessage<'_>,
        seq: u64,
    ) -> Result<OutboundOpaqueMessage, rustls::Error> {
        let mut payload =
            PrefixedPayload::with_capacity(self.encrypted_payload_len(msg.payload.len()));
        payload.extend_from_chunks(&msg.payload);
        let aad = make_tls12_aad(seq, msg.typ, msg.version, msg.payload.len());
        seal_payload(&self.0, seq, &aad, &mut payload)?;
        Ok(OutboundOpaqueMessage::new(msg.typ, msg.version, payload))
    }

    fn encrypted_payload_len(&self, payload_len: usize) -> usize {
        payload_len + TAG_LEN
    }
}

impl MessageDecrypter for Tls12Decrypter {
    fn decrypt<'a>(
        &mut self,
        mut msg: InboundOpaqueMessage<'a>,
        seq: u64,
    ) -> Result<InboundPlainMessage<'a>, rustls::Error> {
        let plain_len = msg
            .payload
            .len()
            .checked_sub(TAG_LEN)
            .ok_or(rustls::Error::DecryptError)?;
        let aad = make_tls12_aad(seq, msg.typ, msg.version, plain_len);
        open_payload(&self.0, seq, &aad, &mut msg.payload)?;
        if plain_len > MAX_FRAGMENT_LEN {
            return Err(rustls::Error::PeerSentOversizedRecord);
        }
        msg.payload.truncate(plain_len);
        Ok(msg.into_plain_message())
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde::Deserialize;

    use super::*;
    use crate::testutil::{Expect, hex, hex_array, wycheproof};

    const SUNSCREEN: &[u8] = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";

    #[test]
    fn rfc8439_2_6_2_one_time_key() {
        let key = hex_array("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f");
        let one_time = poly1305_key(&key, &hex_array("000000000001020304050607"));
        let expected = "8ad5a08b905f81cc815040274ab29471a833b637e3fd0da508dbb8e2fdd1a646";
        assert_eq!(one_time.to_vec(), hex(expected));
    }

    #[test]
    fn rfc8439_2_8_2_seal() {
        let key = hex_array("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f");
        let nonce = hex_array("070000004041424344454647");
        let aad = hex("50515253c0c1c2c3c4c5c6c7");
        let mut data = SUNSCREEN.to_vec();
        let tag = seal(&key, &nonce, &aad, &mut data).unwrap();
        let expected = concat!(
            "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6",
            "3dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b36",
            "92ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc",
            "3ff4def08e4b7a9de576d26586cec64b6116",
        );
        assert_eq!(data, hex(expected));
        assert_eq!(tag.to_vec(), hex("1ae10b594f09e26a7e902ecbd0600691"));
        open(&key, &nonce, &aad, &mut data, &tag).unwrap();
        assert_eq!(data, SUNSCREEN);
    }

    #[test]
    fn rfc8439_appendix_a4_one_time_keys() {
        for (key, nonce, one_time) in A4 {
            assert_eq!(
                poly1305_key(&hex_array(key), &hex_array(nonce)).to_vec(),
                hex(one_time)
            );
        }
    }

    #[test]
    fn rfc8439_appendix_a5_open() {
        let (key, nonce, aad, ciphertext, tag, plaintext) = A5;
        let mut data = hex(ciphertext);
        open(
            &hex_array(key),
            &hex_array(nonce),
            &hex(aad),
            &mut data,
            &hex_array(tag),
        )
        .unwrap();
        assert_eq!(data, hex(plaintext));
    }

    #[test]
    fn a_wrong_tag_leaves_the_ciphertext_alone() {
        let (key, nonce) = ([7u8; KEY_LEN], [9u8; NONCE_LEN]);
        let mut data = SUNSCREEN.to_vec();
        let mut tag = seal(&key, &nonce, b"aad", &mut data).unwrap();
        let sealed = data.clone();
        tag[15] ^= 0x80;
        assert_eq!(open(&key, &nonce, b"aad", &mut data, &tag), Err(Error));
        assert_eq!(data, sealed);
    }

    #[derive(Deserialize)]
    struct Group {
        tests: Vec<Case>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        tc_id: u32,
        key: String,
        iv: String,
        aad: String,
        msg: String,
        ct: String,
        tag: String,
        result: Expect,
    }

    /// Wycheproof `chacha20_poly1305_test.json`: valid cases seal to the vector and open again; every other case is
    /// refused (keys and nonces of the wrong length never reach the AEAD: the types make them unrepresentable).
    #[test]
    fn wycheproof_chacha20_poly1305() {
        let file = wycheproof::<Group>("chacha20_poly1305_test.json");
        let mut seen = 0;
        for case in file.test_groups.iter().flat_map(|group| &group.tests) {
            seen += 1;
            let (Ok(key), Ok(nonce), Ok(tag)) = (
                <[u8; KEY_LEN]>::try_from(hex(&case.key)),
                <[u8; NONCE_LEN]>::try_from(hex(&case.iv)),
                <[u8; TAG_LEN]>::try_from(hex(&case.tag)),
            ) else {
                assert_ne!(case.result, Expect::Valid, "case {}", case.tc_id);
                continue;
            };
            let aad = hex(&case.aad);
            let mut data = hex(&case.ct);
            let opened = open(&key, &nonce, &aad, &mut data, &tag);
            match case.result {
                Expect::Valid => {
                    assert!(opened.is_ok(), "case {} refused", case.tc_id);
                    assert_eq!(data, hex(&case.msg), "case {}", case.tc_id);
                    let sealed = seal(&key, &nonce, &aad, &mut data).unwrap();
                    assert_eq!(data, hex(&case.ct), "case {}", case.tc_id);
                    assert_eq!(sealed, tag, "case {}", case.tc_id);
                }
                Expect::Invalid | Expect::Acceptable => {
                    assert_eq!(opened, Err(Error), "case {} accepted", case.tc_id);
                }
            }
        }
        assert_eq!(seen, file.number_of_tests);
    }

    proptest! {
        /// Random input never panics `open`, and a random tag is refused.
        #[test]
        fn open_refuses_random_input(
            key in any::<[u8; KEY_LEN]>(),
            nonce in any::<[u8; NONCE_LEN]>(),
            aad in proptest::collection::vec(any::<u8>(), 0..64),
            mut data in proptest::collection::vec(any::<u8>(), 0..300),
            tag in any::<[u8; TAG_LEN]>(),
        ) {
            prop_assert_eq!(open(&key, &nonce, &aad, &mut data, &tag), Err(Error));
        }

        /// What `seal` produces, `open` restores; one flipped bit anywhere is refused.
        #[test]
        fn seal_open_round_trip(
            key in any::<[u8; KEY_LEN]>(),
            nonce in any::<[u8; NONCE_LEN]>(),
            aad in proptest::collection::vec(any::<u8>(), 0..64),
            plain in proptest::collection::vec(any::<u8>(), 1..300),
            flip in any::<proptest::sample::Index>(),
        ) {
            let mut data = plain.clone();
            let tag = seal(&key, &nonce, &aad, &mut data).unwrap();
            let mut flipped = data.clone();
            flipped[flip.index(plain.len())] ^= 1;
            prop_assert_eq!(open(&key, &nonce, &aad, &mut flipped, &tag), Err(Error));
            open(&key, &nonce, &aad, &mut data, &tag).unwrap();
            prop_assert_eq!(data, plain);
        }

        /// Cross-check against RustCrypto's `chacha20poly1305` (`docs/plan-tls.md` §7.2).
        #[test]
        fn matches_chacha20poly1305(
            key in any::<[u8; KEY_LEN]>(),
            nonce in any::<[u8; NONCE_LEN]>(),
            aad in proptest::collection::vec(any::<u8>(), 0..64),
            plain in proptest::collection::vec(any::<u8>(), 0..300),
        ) {
            use chacha20poly1305::{AeadInOut, ChaCha20Poly1305, KeyInit};
            let oracle = ChaCha20Poly1305::new(&key.into());
            let mut expected = plain.clone();
            let expected_tag = oracle.encrypt_inout_detached(&nonce.into(), &aad, expected.as_mut_slice().into()).unwrap();
            let mut data = plain;
            let tag = seal(&key, &nonce, &aad, &mut data).unwrap();
            prop_assert_eq!(data, expected);
            prop_assert_eq!(tag.as_slice(), expected_tag.as_slice());
        }
    }

    /// RFC 8439 Appendix A.4: (key, nonce, one-time key).
    const A4: &[(&str, &str, &str)] = &[
        (
            "0000000000000000000000000000000000000000000000000000000000000000",
            "000000000000000000000000",
            "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7",
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000000001",
            "000000000000000000000002",
            "ecfa254f845f647473d3cb140da9e87606cb33066c447b87bc2666dde3fbb739",
        ),
        (
            "1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0",
            "000000000000000000000002",
            "965e3bc6f9ec7ed9560808f4d229f94b137ff275ca9b3fcbdd59deaad23310ae",
        ),
    ];
    /// RFC 8439 Appendix A.5: (key, nonce, aad, ciphertext, tag, plaintext).
    const A5: (&str, &str, &str, &str, &str, &str) = (
        "1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0",
        "000000000102030405060708",
        "f33388860000000000004e91",
        concat!(
            "64a0861575861af460f062c79be643bd5e805cfd345cf389f108670ac76c8cb24c6cfc18755d43eea09ee94e382d26b0",
            "bdb7b73c321b0100d4f03b7f355894cf332f830e710b97ce98c8a84abd0b948114ad176e008d33bd60f982b1ff37c855",
            "9797a06ef4f0ef61c186324e2b3506383606907b6a7c02b0f9f6157b53c867e4b9166c767b804d46a59b5216cde7a4e9",
            "9040c5a40433225ee282a1b0a06c523eaf4534d7f83fa1155b0047718cbc546a0d072b04b3564eea1b422273f548271a",
            "0bb2316053fa76991955ebd63159434ecebb4e466dae5a1073a6727627097a1049e617d91d361094fa68f0ff77987130",
            "305beaba2eda04df997b714d6c6f2c29a6ad5cb4022b02709b",
        ),
        "eead9d67890cbb22392336fea1851f38",
        concat!(
            "496e7465726e65742d4472616674732061726520647261667420646f63756d656e74732076616c696420666f72206120",
            "6d6178696d756d206f6620736978206d6f6e74687320616e64206d617920626520757064617465642c207265706c6163",
            "65642c206f72206f62736f6c65746564206279206f7468657220646f63756d656e747320617420616e792074696d652e",
            "20497420697320696e617070726f70726961746520746f2075736520496e7465726e65742d4472616674732061732072",
            "65666572656e6365206d6174657269616c206f7220746f2063697465207468656d206f74686572207468616e20617320",
            "2fe2809c776f726b20696e2070726f67726573732e2fe2809d",
        ),
    );
}
