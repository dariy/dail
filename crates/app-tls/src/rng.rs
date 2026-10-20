//! The OS random source, and the key provider that loads no keys.

use std::sync::Arc;

use rustls::crypto::{GetRandomFailed, KeyProvider, SecureRandom};
use rustls::pki_types::PrivateKeyDer;
use rustls::sign::SigningKey;

/// Random bytes from the operating system (`getrandom`).
#[derive(Debug)]
pub(crate) struct OsRandom;

impl SecureRandom for OsRandom {
    fn fill(&self, buf: &mut [u8]) -> Result<(), GetRandomFailed> {
        getrandom::fill(buf).map_err(|_| GetRandomFailed)
    }
}

/// Refuses every private key: dail is a TLS client without client certificates, so it never signs.
#[derive(Debug)]
pub(crate) struct NoKeys;

impl KeyProvider for NoKeys {
    fn load_private_key(
        &self,
        _key: PrivateKeyDer<'static>,
    ) -> Result<Arc<dyn SigningKey>, rustls::Error> {
        Err(rustls::Error::General(
            "app-tls loads no private keys: dail never presents a certificate".into(),
        ))
    }
}
