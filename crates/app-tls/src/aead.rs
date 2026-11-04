//! The record protection of TLS 1.3 (RFC 8446 §5.2) and TLS 1.2 over three AEADs from RustCrypto (ADR 0014):
//! ChaCha20-Poly1305 (`chacha20poly1305`, RFC 8439, RFC 7905) and AES-128-GCM and AES-256-GCM (`aes-gcm`,
//! RFC 5288). Only the TLS framing is dail's: nonces, additional data, the explicit nonce of TLS 1.2 GCM.

use aes_gcm::{Aes128Gcm, Aes256Gcm};
use chacha20poly1305::ChaCha20Poly1305 as ChaCha;
use chacha20poly1305::aead::{AeadInOut, KeyInit};
use rustls::crypto::cipher::{
    AeadKey, InboundOpaqueMessage, InboundPlainMessage, Iv, KeyBlockShape, MessageDecrypter,
    MessageEncrypter, Nonce, OutboundOpaqueMessage, OutboundPlainMessage, PrefixedPayload,
    Tls12AeadAlgorithm, Tls13AeadAlgorithm, UnsupportedOperationError, make_tls12_aad,
    make_tls13_aad,
};
use rustls::{ConnectionTrafficSecrets, ContentType, ProtocolVersion};
use zeroize::Zeroize;

use crate::Error;

/// The longest key (ChaCha20, AES-256) in bytes.
#[cfg(test)]
pub(crate) const KEY_LEN: usize = 32;
/// Nonce length in bytes, for all three.
pub(crate) const NONCE_LEN: usize = 12;
/// Tag length in bytes, for all three.
pub(crate) const TAG_LEN: usize = 16;
/// TLS 1.2 GCM: the implicit part of the nonce from the key block, and the explicit part sent in each record
/// (RFC 5288 §3).
const GCM_FIXED_IV_LEN: usize = 4;
const GCM_EXPLICIT_LEN: usize = 8;

/// The largest plaintext in a TLS record (RFC 8446 §5.1, RFC 5246 §6.2.1).
const MAX_FRAGMENT_LEN: usize = 16_384;

/// One of the three AEADs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cipher {
    ChaCha20Poly1305,
    Aes128Gcm,
    Aes256Gcm,
}

impl Cipher {
    /// Key length in bytes.
    pub(crate) const fn key_len(self) -> usize {
        match self {
            Self::Aes128Gcm => 16,
            Self::ChaCha20Poly1305 | Self::Aes256Gcm => 32,
        }
    }
}

/// Encrypts `in_out` in place with `cipher` and returns the tag. A key of the wrong length is refused.
pub(crate) fn seal_with(
    cipher: Cipher,
    key: &[u8],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    in_out: &mut [u8],
) -> Result<[u8; TAG_LEN], Error> {
    fn run<A: AeadInOut + KeyInit>(
        key: &[u8],
        nonce: &[u8; NONCE_LEN],
        aad: &[u8],
        in_out: &mut [u8],
    ) -> Result<[u8; TAG_LEN], Error> {
        let aead = A::new_from_slice(key).map_err(|_| Error)?;
        let nonce = nonce.as_slice().try_into().map_err(|_| Error)?;
        let tag = aead
            .encrypt_inout_detached(nonce, aad, in_out.into())
            .map_err(|_| Error)?;
        tag.as_slice().try_into().map_err(|_| Error)
    }
    match cipher {
        Cipher::ChaCha20Poly1305 => run::<ChaCha>(key, nonce, aad, in_out),
        Cipher::Aes128Gcm => run::<Aes128Gcm>(key, nonce, aad, in_out),
        Cipher::Aes256Gcm => run::<Aes256Gcm>(key, nonce, aad, in_out),
    }
}

/// Checks `tag` over `aad` and the ciphertext `in_out`, then decrypts `in_out` in place. The crates compare the tag
/// in constant time before decrypting; on a mismatch `in_out` is left as it was.
pub(crate) fn open_with(
    cipher: Cipher,
    key: &[u8],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    in_out: &mut [u8],
    tag: &[u8; TAG_LEN],
) -> Result<(), Error> {
    fn run<A: AeadInOut + KeyInit>(
        key: &[u8],
        nonce: &[u8; NONCE_LEN],
        aad: &[u8],
        in_out: &mut [u8],
        tag: &[u8; TAG_LEN],
    ) -> Result<(), Error> {
        let aead = A::new_from_slice(key).map_err(|_| Error)?;
        let nonce = nonce.as_slice().try_into().map_err(|_| Error)?;
        let tag = tag.as_slice().try_into().map_err(|_| Error)?;
        aead.decrypt_inout_detached(nonce, aad, in_out.into(), tag)
            .map_err(|_| Error)
    }
    match cipher {
        Cipher::ChaCha20Poly1305 => run::<ChaCha>(key, nonce, aad, in_out, tag),
        Cipher::Aes128Gcm => run::<Aes128Gcm>(key, nonce, aad, in_out, tag),
        Cipher::Aes256Gcm => run::<Aes256Gcm>(key, nonce, aad, in_out, tag),
    }
}

/// An AEAD as the record protection of TLS 1.3 and TLS 1.2 (rustls's `Tls13AeadAlgorithm`, `Tls12AeadAlgorithm`).
pub(crate) struct Aead(pub(crate) Cipher);

pub(crate) static CHACHA20_POLY1305: Aead = Aead(Cipher::ChaCha20Poly1305);
pub(crate) static AES_128_GCM: Aead = Aead(Cipher::Aes128Gcm);
pub(crate) static AES_256_GCM: Aead = Aead(Cipher::Aes256Gcm);

/// A record key with its cipher and nonce base. The key is wiped on drop. A key or IV of the wrong length can't reach
/// here through rustls (it asks for `key_len` bytes); if one did, the key stays empty and every record fails instead
/// of panicking.
struct RecordKey {
    cipher: Cipher,
    key: Vec<u8>,
    iv: Option<Iv>,
}

impl RecordKey {
    fn new(cipher: Cipher, key: &AeadKey, iv: Option<Iv>) -> Self {
        let key = key.as_ref();
        Self {
            cipher,
            key: if key.len() == cipher.key_len() {
                key.to_vec()
            } else {
                Vec::new()
            },
            iv,
        }
    }

    /// The record nonce: the IV XOR the sequence number (RFC 8446 §5.3, RFC 7905 §2; for TLS 1.2 GCM the IV is the
    /// fixed part and the first explicit nonce, as rustls's own providers build it).
    fn nonce(&self, seq: u64) -> Option<[u8; NONCE_LEN]> {
        Some(Nonce::new(self.iv.as_ref()?, seq).0)
    }

    fn seal(
        &self,
        seq: u64,
        aad: &[u8],
        in_out: &mut [u8],
    ) -> Result<[u8; TAG_LEN], rustls::Error> {
        let nonce = self.nonce(seq).ok_or(rustls::Error::EncryptError)?;
        seal_with(self.cipher, &self.key, &nonce, aad, in_out)
            .map_err(|Error| rustls::Error::EncryptError)
    }
}

impl Drop for RecordKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl Tls13AeadAlgorithm for Aead {
    fn encrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageEncrypter> {
        Box::new(Tls13Encrypter(RecordKey::new(self.0, &key, Some(iv))))
    }

    fn decrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageDecrypter> {
        Box::new(Tls13Decrypter(RecordKey::new(self.0, &key, Some(iv))))
    }

    fn key_len(&self) -> usize {
        self.0.key_len()
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

impl Tls12AeadAlgorithm for Aead {
    fn encrypter(&self, key: AeadKey, iv: &[u8], explicit: &[u8]) -> Box<dyn MessageEncrypter> {
        match self.0 {
            Cipher::ChaCha20Poly1305 => {
                Box::new(Tls12Encrypter(RecordKey::new(self.0, &key, full_iv(iv))))
            }
            Cipher::Aes128Gcm | Cipher::Aes256Gcm => Box::new(GcmEncrypter(RecordKey::new(
                self.0,
                &key,
                gcm_iv(iv, explicit),
            ))),
        }
    }

    fn decrypter(&self, key: AeadKey, iv: &[u8]) -> Box<dyn MessageDecrypter> {
        match self.0 {
            Cipher::ChaCha20Poly1305 => {
                Box::new(Tls12Decrypter(RecordKey::new(self.0, &key, full_iv(iv))))
            }
            // The explicit part comes with each record; the fixed part is kept here.
            Cipher::Aes128Gcm | Cipher::Aes256Gcm => Box::new(GcmDecrypter(RecordKey::new(
                self.0,
                &key,
                gcm_iv(iv, &[0; GCM_EXPLICIT_LEN]),
            ))),
        }
    }

    /// RFC 7905 §2: a 12-byte fixed IV and no explicit nonce; RFC 5288 §3: a 4-byte salt and an 8-byte explicit nonce.
    fn key_block_shape(&self) -> KeyBlockShape {
        match self.0 {
            Cipher::ChaCha20Poly1305 => KeyBlockShape {
                enc_key_len: self.0.key_len(),
                fixed_iv_len: NONCE_LEN,
                explicit_nonce_len: 0,
            },
            Cipher::Aes128Gcm | Cipher::Aes256Gcm => KeyBlockShape {
                enc_key_len: self.0.key_len(),
                fixed_iv_len: GCM_FIXED_IV_LEN,
                explicit_nonce_len: GCM_EXPLICIT_LEN,
            },
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

fn full_iv(iv: &[u8]) -> Option<Iv> {
    <[u8; NONCE_LEN]>::try_from(iv).ok().map(Iv::new)
}

/// The TLS 1.2 GCM nonce base: the 4-byte salt, then the 8-byte explicit part.
fn gcm_iv(salt: &[u8], explicit: &[u8]) -> Option<Iv> {
    if salt.len() != GCM_FIXED_IV_LEN || explicit.len() != GCM_EXPLICIT_LEN {
        return None;
    }
    let mut iv = [0u8; NONCE_LEN];
    iv[..GCM_FIXED_IV_LEN].copy_from_slice(salt);
    iv[GCM_FIXED_IV_LEN..].copy_from_slice(explicit);
    Some(Iv::new(iv))
}

/// Checks the tag at the end of `payload` and decrypts the rest in place with `nonce`; returns the plaintext length.
fn open_payload(
    key: &RecordKey,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    payload: &mut [u8],
) -> Result<usize, rustls::Error> {
    let plain_len = payload
        .len()
        .checked_sub(TAG_LEN)
        .ok_or(rustls::Error::DecryptError)?;
    let (ciphertext, tag) = payload.split_at_mut(plain_len);
    let tag: &[u8; TAG_LEN] = (&*tag)
        .try_into()
        .map_err(|_| rustls::Error::DecryptError)?;
    open_with(key.cipher, &key.key, nonce, aad, ciphertext, tag)
        .map_err(|Error| rustls::Error::DecryptError)?;
    Ok(plain_len)
}

struct Tls13Encrypter(RecordKey);
struct Tls13Decrypter(RecordKey);
struct Tls12Encrypter(RecordKey);
struct Tls12Decrypter(RecordKey);
struct GcmEncrypter(RecordKey);
struct GcmDecrypter(RecordKey);

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
        let tag = self
            .0
            .seal(seq, &make_tls13_aad(total_len), payload.as_mut())?;
        payload.extend_from_slice(&tag);
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
        let nonce = self.0.nonce(seq).ok_or(rustls::Error::DecryptError)?;
        let plain_len = open_payload(&self.0, &nonce, &aad, &mut msg.payload)?;
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
        let tag = self.0.seal(seq, &aad, payload.as_mut())?;
        payload.extend_from_slice(&tag);
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
        let nonce = self.0.nonce(seq).ok_or(rustls::Error::DecryptError)?;
        open_payload(&self.0, &nonce, &aad, &mut msg.payload)?;
        if plain_len > MAX_FRAGMENT_LEN {
            return Err(rustls::Error::PeerSentOversizedRecord);
        }
        msg.payload.truncate(plain_len);
        Ok(msg.into_plain_message())
    }
}

impl MessageEncrypter for GcmEncrypter {
    /// RFC 5288 §3: the record is the explicit nonce (the nonce's last 8 bytes), the ciphertext and the tag; the
    /// additional data is as for any TLS 1.2 AEAD.
    fn encrypt(
        &mut self,
        msg: OutboundPlainMessage<'_>,
        seq: u64,
    ) -> Result<OutboundOpaqueMessage, rustls::Error> {
        let nonce = self.0.nonce(seq).ok_or(rustls::Error::EncryptError)?;
        let mut payload =
            PrefixedPayload::with_capacity(self.encrypted_payload_len(msg.payload.len()));
        payload.extend_from_slice(&nonce[GCM_FIXED_IV_LEN..]);
        payload.extend_from_chunks(&msg.payload);
        let aad = make_tls12_aad(seq, msg.typ, msg.version, msg.payload.len());
        let tag = seal_with(
            self.0.cipher,
            &self.0.key,
            &nonce,
            &aad,
            &mut payload.as_mut()[GCM_EXPLICIT_LEN..],
        )
        .map_err(|Error| rustls::Error::EncryptError)?;
        payload.extend_from_slice(&tag);
        Ok(OutboundOpaqueMessage::new(msg.typ, msg.version, payload))
    }

    fn encrypted_payload_len(&self, payload_len: usize) -> usize {
        GCM_EXPLICIT_LEN + payload_len + TAG_LEN
    }
}

impl MessageDecrypter for GcmDecrypter {
    fn decrypt<'a>(
        &mut self,
        mut msg: InboundOpaqueMessage<'a>,
        seq: u64,
    ) -> Result<InboundPlainMessage<'a>, rustls::Error> {
        let plain_len = msg
            .payload
            .len()
            .checked_sub(GCM_EXPLICIT_LEN + TAG_LEN)
            .ok_or(rustls::Error::DecryptError)?;
        let mut nonce = self.0.nonce(0).ok_or(rustls::Error::DecryptError)?;
        nonce[GCM_FIXED_IV_LEN..].copy_from_slice(&msg.payload[..GCM_EXPLICIT_LEN]);
        let aad = make_tls12_aad(seq, msg.typ, msg.version, plain_len);
        open_payload(&self.0, &nonce, &aad, &mut msg.payload[GCM_EXPLICIT_LEN..])?;
        if plain_len > MAX_FRAGMENT_LEN {
            return Err(rustls::Error::PeerSentOversizedRecord);
        }
        // Drop the explicit nonce in front and the tag behind.
        msg.payload
            .copy_within(GCM_EXPLICIT_LEN..GCM_EXPLICIT_LEN + plain_len, 0);
        msg.payload.truncate(plain_len);
        Ok(msg.into_plain_message())
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde::Deserialize;

    use super::*;

    /// ChaCha20-Poly1305 with 32-byte keys, as the RFC 8439 and Wycheproof vectors use it.
    fn seal(
        key: &[u8; KEY_LEN],
        nonce: &[u8; NONCE_LEN],
        aad: &[u8],
        in_out: &mut [u8],
    ) -> Result<[u8; TAG_LEN], Error> {
        seal_with(Cipher::ChaCha20Poly1305, key, nonce, aad, in_out)
    }

    fn open(
        key: &[u8; KEY_LEN],
        nonce: &[u8; NONCE_LEN],
        aad: &[u8],
        in_out: &mut [u8],
        tag: &[u8; TAG_LEN],
    ) -> Result<(), Error> {
        open_with(Cipher::ChaCha20Poly1305, key, nonce, aad, in_out, tag)
    }
    use crate::testutil::{Expect, hex, hex_array, wycheproof};

    const SUNSCREEN: &[u8] = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";

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

    }

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
