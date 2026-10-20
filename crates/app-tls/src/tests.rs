//! Tests of the assembled provider (`docs/plan-tls.md` §5.8, §7.4): the pinned algorithm set, and TLS handshakes
//! between a rustls client and a rustls server in memory, both on `provider()`. The server signs with a test-only key
//! provider built on `bignum`, `ec` and `der`, which exists only here.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rustls::client::WebPkiServerVerifier;
use rustls::crypto::{
    ActiveKeyExchange, CryptoProvider, KeyProvider, SharedSecret, SupportedKxGroup,
};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::sign::{Signer, SigningKey};
use rustls::time_provider::TimeProvider;
use rustls::version::{TLS12, TLS13};
use rustls::{
    CertificateError, CipherSuite, ClientConfig, ClientConnection, NamedGroup, PeerMisbehaved,
    ProtocolVersion, RootCertStore, ServerConfig, ServerConnection, SignatureAlgorithm,
    SignatureScheme, SupportedProtocolVersion,
};

use crate::bignum::{self, Modulus};
use crate::der::Reader;
use crate::ec::{self, CurveId};
use crate::hash::SigHash;
use crate::{provider, rsa};

#[test]
fn the_provider_is_pinned() {
    let p = provider();
    let suites: Vec<CipherSuite> = p.cipher_suites.iter().map(|suite| suite.suite()).collect();
    assert_eq!(
        suites,
        [
            CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
            CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
            CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
        ]
    );
    let groups: Vec<NamedGroup> = p.kx_groups.iter().map(|group| group.name()).collect();
    assert_eq!(groups, [NamedGroup::X25519]);

    let algorithms = p.signature_verification_algorithms;
    let ids: Vec<String> = algorithms
        .all
        .iter()
        .map(|alg| {
            format!(
                "{:02x?} {:02x?}",
                alg.public_key_alg_id().as_ref(),
                alg.signature_alg_id().as_ref()
            )
        })
        .collect();
    assert_eq!(ids.len(), 13);
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 13, "an algorithm is listed twice");
    let mapping: Vec<(SignatureScheme, usize)> = algorithms
        .mapping
        .iter()
        .map(|(scheme, algs)| (*scheme, algs.len()))
        .collect();
    assert_eq!(
        mapping,
        [
            (SignatureScheme::ECDSA_NISTP384_SHA384, 2),
            (SignatureScheme::ECDSA_NISTP256_SHA256, 2),
            (SignatureScheme::RSA_PSS_SHA512, 1),
            (SignatureScheme::RSA_PSS_SHA384, 1),
            (SignatureScheme::RSA_PSS_SHA256, 1),
            (SignatureScheme::RSA_PKCS1_SHA512, 1),
            (SignatureScheme::RSA_PKCS1_SHA384, 1),
            (SignatureScheme::RSA_PKCS1_SHA256, 1),
        ]
    );
    // Every mapped algorithm is in `all`.
    for (_, algs) in algorithms.mapping {
        for alg in *algs {
            let id = format!(
                "{:02x?} {:02x?}",
                alg.public_key_alg_id().as_ref(),
                alg.signature_alg_id().as_ref()
            );
            assert!(ids.contains(&id), "{id}");
        }
    }

    let key = PrivateKeyDer::from_pem_file(cert_path("server-ecdsa.key")).unwrap();
    assert!(p.key_provider.load_private_key(key).is_err());
    let mut random = [0u8; 32];
    p.secure_random.fill(&mut random).unwrap();
    assert_ne!(random, [0u8; 32]);
}

// --- Fixtures ---------------------------------------------------------------------------------------------------

fn cert_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/certs")
        .join(name)
}

fn cert(name: &str) -> CertificateDer<'static> {
    CertificateDer::from_pem_file(cert_path(name)).unwrap()
}

/// The certificate chains of `testdata/certs` (`docs/plan-tls.md` Appendix B).
#[derive(Clone, Copy)]
enum Chain {
    /// A P-384 root signs a P-256 server certificate with SHA-384, as in Gmail's chain.
    Ecdsa,
    /// A 3072-bit root signs a 2048-bit server certificate with SHA-256, as in Hostinger's chain.
    Rsa,
}

impl Chain {
    fn name(self) -> &'static str {
        match self {
            Self::Ecdsa => "ecdsa",
            Self::Rsa => "rsa",
        }
    }

    fn root(self) -> CertificateDer<'static> {
        cert(&format!("root-{}.pem", self.name()))
    }

    fn server_cert(self) -> CertificateDer<'static> {
        cert(&format!("server-{}.pem", self.name()))
    }

    fn server_key(self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::from_pem_file(cert_path(&format!("server-{}.key", self.name()))).unwrap()
    }
}

// --- The test-only key provider ---------------------------------------------------------------------------------

/// Loads the PKCS#8 test keys and signs with the schemes in `rsa_schemes` (for RSA keys) or ECDSA P-256 with
/// SHA-256. Signing is variable time and the ECDSA nonce is derived from key and message: tests only.
#[derive(Debug)]
struct TestKeys {
    rsa_schemes: &'static [SignatureScheme],
}

static RSA_PSS_AND_PKCS1: TestKeys = TestKeys {
    rsa_schemes: &[
        SignatureScheme::RSA_PSS_SHA256,
        SignatureScheme::RSA_PKCS1_SHA256,
    ],
};
static RSA_PKCS1_ONLY: TestKeys = TestKeys {
    rsa_schemes: &[SignatureScheme::RSA_PKCS1_SHA256],
};

const OID_RSA_ENCRYPTION: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
const OID_EC_PUBLIC_KEY: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];

#[derive(Debug)]
enum TestKey {
    /// n, d.
    Rsa(Vec<u8>, Vec<u8>, &'static [SignatureScheme]),
    /// The P-256 private scalar.
    Ecdsa(Vec<u8>),
}

impl KeyProvider for TestKeys {
    fn load_private_key(
        &self,
        key: PrivateKeyDer<'static>,
    ) -> Result<Arc<dyn SigningKey>, rustls::Error> {
        let PrivateKeyDer::Pkcs8(pkcs8) = key else {
            return Err(rustls::Error::General("PKCS#8 only".into()));
        };
        // PrivateKeyInfo { version, AlgorithmIdentifier { oid, params }, privateKey OCTET STRING } (RFC 5208 §5).
        let mut outer = Reader::new(pkcs8.secret_pkcs8_der());
        let mut info = outer.sequence().unwrap();
        info.unsigned_integer().unwrap();
        let mut alg = info.sequence().unwrap();
        let oid = alg.tagged(0x06).unwrap();
        let private = info.tagged(0x04).unwrap();
        let key = if oid == OID_RSA_ENCRYPTION {
            // RSAPrivateKey { version, n, e, d, … } (RFC 8017 §A.1.2).
            let mut seq = Reader::new(private).sequence().unwrap();
            seq.unsigned_integer().unwrap();
            let n = seq.unsigned_integer().unwrap().to_vec();
            seq.unsigned_integer().unwrap();
            let d = seq.unsigned_integer().unwrap().to_vec();
            TestKey::Rsa(n, d, self.rsa_schemes)
        } else if oid == OID_EC_PUBLIC_KEY {
            // ECPrivateKey { version, privateKey OCTET STRING, … } (RFC 5915 §3).
            let mut seq = Reader::new(private).sequence().unwrap();
            seq.unsigned_integer().unwrap();
            TestKey::Ecdsa(seq.tagged(0x04).unwrap().to_vec())
        } else {
            return Err(rustls::Error::General("unknown key type".into()));
        };
        Ok(Arc::new(key))
    }
}

impl SigningKey for TestKey {
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>> {
        let own: &[SignatureScheme] = match self {
            Self::Rsa(_, _, schemes) => schemes,
            Self::Ecdsa(_) => &[SignatureScheme::ECDSA_NISTP256_SHA256],
        };
        let scheme = own.iter().find(|scheme| offered.contains(scheme))?;
        Some(Box::new(TestSigner {
            key: Arc::new(self.clone_key()),
            scheme: *scheme,
        }))
    }

    fn algorithm(&self) -> SignatureAlgorithm {
        match self {
            Self::Rsa(..) => SignatureAlgorithm::RSA,
            Self::Ecdsa(_) => SignatureAlgorithm::ECDSA,
        }
    }
}

impl TestKey {
    fn clone_key(&self) -> Self {
        match self {
            Self::Rsa(n, d, schemes) => Self::Rsa(n.clone(), d.clone(), schemes),
            Self::Ecdsa(secret) => Self::Ecdsa(secret.clone()),
        }
    }
}

#[derive(Debug)]
struct TestSigner {
    key: Arc<TestKey>,
    scheme: SignatureScheme,
}

impl Signer for TestSigner {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> {
        Ok(match (&*self.key, self.scheme) {
            (TestKey::Ecdsa(secret), _) => {
                ec::sign_for_tests(CurveId::P256, SigHash::Sha256, secret, message)
            }
            (TestKey::Rsa(n, d, _), SignatureScheme::RSA_PKCS1_SHA256) => rsa_private_op(
                n,
                d,
                &rsa::pkcs1_encoding(SigHash::Sha256, message, n.len()).unwrap(),
            ),
            (TestKey::Rsa(n, d, _), _) => rsa_private_op(n, d, &pss_encoding(n, message)),
        })
    }

    fn scheme(&self) -> SignatureScheme {
        self.scheme
    }
}

/// `em^d mod n`, written into the modulus length.
fn rsa_private_op(n: &[u8], d: &[u8], em: &[u8]) -> Vec<u8> {
    let modulus = Modulus::new(n).unwrap();
    let m = bignum::from_be_bytes(em, modulus.limbs()).unwrap();
    let s = modulus.to_plain(&modulus.pow(&modulus.to_mont(&m), d));
    bignum::to_be_bytes(&s, n.len()).unwrap()
}

/// EMSA-PSS-ENCODE (RFC 8017 §9.1.1) with SHA-256, MGF1-SHA256 and a fixed 32-byte salt.
fn pss_encoding(n: &[u8], message: &[u8]) -> Vec<u8> {
    let hash = SigHash::Sha256;
    let em_bits = bignum::bits(&bignum::from_be_bytes(n, n.len().div_ceil(8)).unwrap()) - 1;
    let em_len = em_bits.div_ceil(8);
    let salt = [0x5au8; 32];
    let m_hash = hash.digest(&[message]);
    let h = hash.digest(&[&[0u8; 8], &m_hash, &salt]);
    let mut db = vec![0u8; em_len - salt.len() - h.len() - 2];
    db.push(0x01);
    db.extend_from_slice(&salt);
    for (byte, mask) in db.iter_mut().zip(rsa::mgf1(hash, &h, em_len - h.len() - 1)) {
        *byte ^= mask;
    }
    db[0] &= 0xff >> (8 * em_len - em_bits);
    [db, h, vec![0xbc]].concat()
}

// --- Connections in memory --------------------------------------------------------------------------------------

/// The server: `provider()` with the test keys, and optionally only `suite`.
fn server(
    chain: Chain,
    keys: &'static TestKeys,
    suite: Option<CipherSuite>,
    kx: Option<&'static dyn SupportedKxGroup>,
) -> ServerConnection {
    let mut p = CryptoProvider {
        key_provider: keys,
        ..provider()
    };
    if let Some(suite) = suite {
        p.cipher_suites.retain(|s| s.suite() == suite);
    }
    if let Some(kx) = kx {
        p.kx_groups = vec![kx];
    }
    let config = ServerConfig::builder_with_provider(Arc::new(p))
        .with_protocol_versions(&[&TLS13, &TLS12])
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![chain.server_cert()], chain.server_key())
        .unwrap();
    let mut conn = ServerConnection::new(Arc::new(config)).unwrap();
    conn.set_buffer_limit(None);
    conn
}

/// A client that trusts `root`, speaks `version`, and asks for `name`.
fn client(
    root: CertificateDer<'static>,
    version: &'static SupportedProtocolVersion,
    name: &str,
) -> ClientConnection {
    client_at(root, version, name, None)
}

fn client_at(
    root: CertificateDer<'static>,
    version: &'static SupportedProtocolVersion,
    name: &str,
    time: Option<Arc<dyn TimeProvider>>,
) -> ClientConnection {
    let mut roots = RootCertStore::empty();
    roots.add(root).unwrap();
    let p = Arc::new(provider());
    let builder = match time {
        Some(time) => ClientConfig::builder_with_details(p, time),
        None => ClientConfig::builder_with_provider(p),
    };
    let config = builder
        .with_protocol_versions(&[version])
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut conn = ClientConnection::new(
        Arc::new(config),
        ServerName::try_from(name.to_string()).unwrap(),
    )
    .unwrap();
    conn.set_buffer_limit(None);
    conn
}

/// Moves records both ways until neither side has anything to send. Plaintext that arrives is appended to
/// `client_in` / `server_in`.
fn pump(
    client: &mut ClientConnection,
    server: &mut ServerConnection,
    client_in: &mut Vec<u8>,
    server_in: &mut Vec<u8>,
) -> Result<(), rustls::Error> {
    loop {
        let mut wire = Vec::new();
        while client.wants_write() {
            client.write_tls(&mut wire).unwrap();
        }
        let client_sent = !wire.is_empty();
        let mut data = &wire[..];
        while !data.is_empty() {
            server.read_tls(&mut data).unwrap();
            server.process_new_packets()?;
            drain(server.reader(), server_in);
        }
        let mut wire = Vec::new();
        while server.wants_write() {
            server.write_tls(&mut wire).unwrap();
        }
        let server_sent = !wire.is_empty();
        let mut data = &wire[..];
        while !data.is_empty() {
            client.read_tls(&mut data).unwrap();
            client.process_new_packets()?;
            drain(client.reader(), client_in);
        }
        if !client_sent && !server_sent {
            return Ok(());
        }
    }
}

fn drain(mut reader: impl Read, into: &mut Vec<u8>) {
    let mut buf = [0u8; 16_384];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => into.extend_from_slice(&buf[..n]),
            Err(err) => {
                assert_eq!(err.kind(), io::ErrorKind::WouldBlock, "{err}");
                return;
            }
        }
    }
}

/// Handshakes, checks what was negotiated, and sends 1 MiB each way.
fn connect_and_exchange(
    mut client: ClientConnection,
    mut server: ServerConnection,
    version: ProtocolVersion,
    suite: CipherSuite,
) {
    let (mut client_in, mut server_in) = (Vec::new(), Vec::new());
    pump(&mut client, &mut server, &mut client_in, &mut server_in).unwrap();
    assert!(!client.is_handshaking() && !server.is_handshaking());
    assert_eq!(client.protocol_version(), Some(version));
    assert_eq!(
        client.negotiated_cipher_suite().map(|s| s.suite()),
        Some(suite)
    );
    assert_eq!(
        client.negotiated_key_exchange_group().map(|g| g.name()),
        Some(NamedGroup::X25519)
    );

    let up: Vec<u8> = (0..1 << 20).map(|i: u32| (i * 7 + i / 251) as u8).collect();
    let down: Vec<u8> = up.iter().rev().copied().collect();
    client.writer().write_all(&up).unwrap();
    server.writer().write_all(&down).unwrap();
    pump(&mut client, &mut server, &mut client_in, &mut server_in).unwrap();
    assert!(
        server_in == up,
        "the server received {} bytes",
        server_in.len()
    );
    assert!(
        client_in == down,
        "the client received {} bytes",
        client_in.len()
    );
}

fn handshake_error(mut client: ClientConnection, mut server: ServerConnection) -> rustls::Error {
    let (mut client_in, mut server_in) = (Vec::new(), Vec::new());
    let result = pump(&mut client, &mut server, &mut client_in, &mut server_in);
    assert!(result.is_err(), "the handshake succeeded");
    result.err().unwrap()
}

// --- Handshakes that must succeed -------------------------------------------------------------------------------

#[test]
fn tls13_with_the_ecdsa_chain() {
    let chain = Chain::Ecdsa;
    let (c, s) = (
        client(chain.root(), &TLS13, "mail.dail.test"),
        server(chain, &RSA_PSS_AND_PKCS1, None, None),
    );
    connect_and_exchange(
        c,
        s,
        ProtocolVersion::TLSv1_3,
        CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
    );
}

#[test]
fn tls13_with_the_rsa_chain() {
    let chain = Chain::Rsa;
    let (c, s) = (
        client(chain.root(), &TLS13, "mail.dail.test"),
        server(chain, &RSA_PSS_AND_PKCS1, None, None),
    );
    connect_and_exchange(
        c,
        s,
        ProtocolVersion::TLSv1_3,
        CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
    );
}

#[test]
fn tls12_ecdhe_ecdsa() {
    let chain = Chain::Ecdsa;
    let suite = CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256;
    let (c, s) = (
        client(chain.root(), &TLS12, "mail.dail.test"),
        server(chain, &RSA_PSS_AND_PKCS1, Some(suite), None),
    );
    connect_and_exchange(c, s, ProtocolVersion::TLSv1_2, suite);
}

#[test]
fn tls12_ecdhe_rsa_with_pss() {
    let chain = Chain::Rsa;
    let suite = CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256;
    let (c, s) = (
        client(chain.root(), &TLS12, "mail.dail.test"),
        server(chain, &RSA_PSS_AND_PKCS1, Some(suite), None),
    );
    connect_and_exchange(c, s, ProtocolVersion::TLSv1_2, suite);
}

#[test]
fn tls12_ecdhe_rsa_with_pkcs1() {
    let chain = Chain::Rsa;
    let suite = CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256;
    let (c, s) = (
        client(chain.root(), &TLS12, "mail.dail.test"),
        server(chain, &RSA_PKCS1_ONLY, Some(suite), None),
    );
    connect_and_exchange(c, s, ProtocolVersion::TLSv1_2, suite);
}

// --- Handshakes and records that must fail ----------------------------------------------------------------------

#[test]
fn an_unknown_issuer_is_refused() {
    let c = client(Chain::Rsa.root(), &TLS13, "mail.dail.test");
    let err = handshake_error(c, server(Chain::Ecdsa, &RSA_PSS_AND_PKCS1, None, None));
    assert_eq!(
        err,
        rustls::Error::InvalidCertificate(CertificateError::UnknownIssuer)
    );
}

#[test]
fn the_wrong_server_name_is_refused() {
    let c = client(Chain::Ecdsa.root(), &TLS13, "other.dail.test");
    let err = handshake_error(c, server(Chain::Ecdsa, &RSA_PSS_AND_PKCS1, None, None));
    assert!(
        matches!(
            err,
            rustls::Error::InvalidCertificate(CertificateError::NotValidForNameContext { .. })
        ),
        "{err:?}"
    );
}

#[derive(Debug)]
struct Year2128;

impl TimeProvider for Year2128 {
    fn current_time(&self) -> Option<UnixTime> {
        Some(UnixTime::since_unix_epoch(Duration::from_secs(
            5_000_000_000,
        )))
    }
}

#[test]
fn an_expired_certificate_is_refused() {
    let c = client_at(
        Chain::Rsa.root(),
        &TLS13,
        "mail.dail.test",
        Some(Arc::new(Year2128)),
    );
    let err = handshake_error(c, server(Chain::Rsa, &RSA_PSS_AND_PKCS1, None, None));
    assert!(
        matches!(
            err,
            rustls::Error::InvalidCertificate(CertificateError::ExpiredContext { .. })
        ),
        "{err:?}"
    );
}

#[test]
fn a_flipped_ciphertext_bit_is_refused() {
    for (chain, version) in [(Chain::Ecdsa, &TLS13), (Chain::Rsa, &TLS12)] {
        let mut c = client(chain.root(), version, "mail.dail.test");
        let mut s = server(chain, &RSA_PSS_AND_PKCS1, None, None);
        let (mut client_in, mut server_in) = (Vec::new(), Vec::new());
        pump(&mut c, &mut s, &mut client_in, &mut server_in).unwrap();
        c.writer().write_all(b"a secret").unwrap();
        let mut wire = Vec::new();
        while c.wants_write() {
            c.write_tls(&mut wire).unwrap();
        }
        let middle = wire.len() - 10;
        wire[middle] ^= 0x01;
        s.read_tls(&mut &wire[..]).unwrap();
        assert_eq!(
            s.process_new_packets().err(),
            Some(rustls::Error::DecryptError)
        );
        drain(s.reader(), &mut server_in);
        assert!(server_in.is_empty());
    }
}

/// A server key share that is a low-order point, so X25519 gives an all-zero secret.
#[derive(Debug)]
struct LowOrderKx;

impl SupportedKxGroup for LowOrderKx {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, rustls::Error> {
        Ok(Box::new(LowOrderShare))
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::X25519
    }
}

struct LowOrderShare;

impl ActiveKeyExchange for LowOrderShare {
    fn complete(self: Box<Self>, _peer: &[u8]) -> Result<SharedSecret, rustls::Error> {
        Ok(SharedSecret::from(&[0u8; 32][..]))
    }

    fn pub_key(&self) -> &[u8] {
        &[0u8; 32]
    }

    fn group(&self) -> NamedGroup {
        NamedGroup::X25519
    }
}

#[test]
fn an_all_zero_x25519_secret_is_refused() {
    for version in [&TLS13, &TLS12] {
        let c = client(Chain::Ecdsa.root(), version, "mail.dail.test");
        let err = handshake_error(
            c,
            server(Chain::Ecdsa, &RSA_PSS_AND_PKCS1, None, Some(&LowOrderKx)),
        );
        assert_eq!(
            err,
            rustls::Error::PeerMisbehaved(PeerMisbehaved::InvalidKeyShare)
        );
    }
}

#[test]
fn the_webpki_verifier_builds_on_the_provider() {
    let mut roots = RootCertStore::empty();
    roots.add(Chain::Ecdsa.root()).unwrap();
    WebPkiServerVerifier::builder_with_provider(Arc::new(roots), Arc::new(provider()))
        .build()
        .unwrap();
}
