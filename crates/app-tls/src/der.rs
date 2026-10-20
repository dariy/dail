//! A strict DER reader for the two structures the verifiers parse: `RSAPublicKey` (RFC 8017 §A.1.1) and the ECDSA
//! signature `SEQUENCE { r INTEGER, s INTEGER }` (RFC 3279 §2.2.3). Only SEQUENCE and INTEGER, definite and minimally
//! encoded lengths, minimal non-negative INTEGERs, and no trailing bytes (X.690 §8.1.3, §8.3, §10.1).

use crate::Error;

const SEQUENCE: u8 = 0x30;
const INTEGER: u8 = 0x02;

/// Reads DER values one after the other from a byte string.
pub(crate) struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { rest: data }
    }

    fn byte(&mut self) -> Result<u8, Error> {
        let (&first, rest) = self.rest.split_first().ok_or(Error)?;
        self.rest = rest;
        Ok(first)
    }

    /// Reads one value with tag `tag` and returns its contents (X.690 §8.1). Long-form lengths must be minimal: at
    /// least 128, without leading zero bytes, at most four bytes.
    fn value(&mut self, tag: u8) -> Result<&'a [u8], Error> {
        if self.byte()? != tag {
            return Err(Error);
        }
        let first = self.byte()?;
        let len = match first {
            0..=0x7f => usize::from(first),
            0x81..=0x84 => {
                let mut len = 0usize;
                for _ in 0..first & 0x7f {
                    len = (len << 8) | usize::from(self.byte()?);
                }
                let min = 1usize << (8 * (usize::from(first & 0x7f) - 1));
                if len < 0x80 || len < min {
                    return Err(Error);
                }
                len
            }
            // 0x80 is the indefinite form, which DER forbids; longer lengths are never needed here.
            _ => return Err(Error),
        };
        if len > self.rest.len() {
            return Err(Error);
        }
        let (contents, rest) = self.rest.split_at(len);
        self.rest = rest;
        Ok(contents)
    }

    /// Reads a SEQUENCE and returns a reader over its contents.
    pub(crate) fn sequence(&mut self) -> Result<Reader<'a>, Error> {
        self.value(SEQUENCE).map(Reader::new)
    }

    /// Reads a non-negative INTEGER and returns its magnitude, big-endian, without the sign byte (X.690 §8.3). Refuses
    /// negative values and non-minimal encodings. Zero comes back empty.
    pub(crate) fn unsigned_integer(&mut self) -> Result<&'a [u8], Error> {
        match self.value(INTEGER)? {
            [] => Err(Error),
            [first, ..] if first & 0x80 != 0 => Err(Error),
            [0] => Ok(&[]),
            [0, second, ..] if second & 0x80 == 0 => Err(Error),
            [0, rest @ ..] => Ok(rest),
            all => Ok(all),
        }
    }

    /// Reads one value with any `tag` and returns its contents: the test-only key loader reads PKCS#8 with it.
    #[cfg(test)]
    pub(crate) fn tagged(&mut self, tag: u8) -> Result<&'a [u8], Error> {
        self.value(tag)
    }

    /// Succeeds when every byte was read.
    pub(crate) fn finish(self) -> Result<(), Error> {
        if self.rest.is_empty() {
            Ok(())
        } else {
            Err(Error)
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::testutil::hex;

    fn pair(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>), Error> {
        let mut outer = Reader::new(data);
        let mut seq = outer.sequence()?;
        let a = seq.unsigned_integer()?.to_vec();
        let b = seq.unsigned_integer()?.to_vec();
        seq.finish()?;
        outer.finish()?;
        Ok((a, b))
    }

    #[test]
    fn reads_a_pair_of_integers() {
        assert_eq!(pair(&hex("3007020101020200ff")), Ok((vec![1], vec![0xff])));
        assert_eq!(pair(&hex("3006020100020100")), Ok((vec![], vec![])));
        let long = format!("3081{:02x}028180{}020103", 3 + 0x80 + 3, "11".repeat(0x80));
        assert_eq!(pair(&hex(&long)), Ok((vec![0x11; 0x80], vec![3])));
        let longer = format!(
            "3082{:04x}0282010000{}020103",
            4 + 0x100 + 3,
            "ff".repeat(0xff)
        );
        assert_eq!(pair(&hex(&longer)), Ok((vec![0xff; 0xff], vec![3])));
    }

    #[test]
    fn refuses_what_der_forbids() {
        for bad in [
            "",                       // nothing
            "3007020101020200ff00",   // trailing byte after the sequence
            "3008020101020200ff00",   // trailing byte inside the sequence
            "3005020101020200ff",     // sequence shorter than its contents
            "3080020101020200ff0000", // indefinite length
            "308107020101020200ff",   // long form for a short length
            "30820007020101020200ff", // long form with a leading zero byte
            "3007020181020200ff",     // negative integer
            "30070202000102017f",     // non-minimal integer (redundant zero)
            "30060202ff80020101",     // non-minimal negative integer
            "3004020002020001",       // empty integer
            "3106020101020200ff",     // SET instead of SEQUENCE
            "3006030101020200ff",     // BIT STRING instead of INTEGER
            "300602010102",           // truncated
        ] {
            assert_eq!(pair(&hex(bad)), Err(Error), "{bad}");
        }
    }

    proptest! {
        /// Random bytes never panic the reader.
        #[test]
        fn random_input_never_panics(data in proptest::collection::vec(any::<u8>(), 0..64)) {
            let _ = pair(&data);
        }
    }
}
