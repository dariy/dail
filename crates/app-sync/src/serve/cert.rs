//! The dail server's identity (ADR 0017): a P-256 key generated once and a self-signed X.509 v3
//! certificate, both in `<profile>/serve/` with user-only permissions.
//!
//! - `serve/key.p256`: the private scalar, 32 raw bytes (big-endian).
//! - `serve/cert.der`: the certificate, DER. Subject `CN=dail server`, valid 20 years from its
//!   creation, an end-entity certificate for TLS servers (`serverAuth`), naming the advertised
//!   hosts and `localhost`, `127.0.0.1` and `::1`.
//!
//! Devices pin the certificate's SHA-256 fingerprint and check nothing else, so the certificate
//! is kept across restarts even when the advertised hosts change: a new one would change the
//! fingerprint and every device would have to pair again. It is made again only when it is
//! missing or does not belong to the key.

use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use p256::ecdsa::{DerSignature, SigningKey};
use rustls::pki_types::CertificateDer;
use rustls::server::ResolvesServerCert;
use rustls::sign::{CertifiedKey, SingleCertAndKey};
use rustls::{ServerConfig, SignatureAlgorithm, SignatureScheme};
use x509_cert::builder::profile::BuilderProfile;
use x509_cert::builder::{Builder as _, CertificateBuilder};
use x509_cert::certificate::TbsCertificate;
use x509_cert::der::asn1::Ia5String;
use x509_cert::der::oid::db::rfc5280::ID_KP_SERVER_AUTH;
use x509_cert::der::{Decode as _, Encode as _};
use x509_cert::ext::Extension;
use x509_cert::ext::ToExtension as _;
use x509_cert::ext::pkix::name::GeneralName;
use x509_cert::ext::pkix::{
    BasicConstraints, ExtendedKeyUsage, KeyUsage, KeyUsages, SubjectAltName, SubjectKeyIdentifier,
};
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::SubjectPublicKeyInfoRef;
use x509_cert::time::{Time, Validity};
use x509_cert::{Certificate, SubjectPublicKeyInfo};

use crate::error::SyncError;

/// How long a new certificate is valid. Devices pin it and ignore its dates; the dates are for
/// the tools that show it.
pub const CERT_VALIDITY: Duration = Duration::from_secs(20 * 365 * 24 * 60 * 60);

const KEY_FILE: &str = "key.p256";
const CERT_FILE: &str = "cert.der";

/// The key and certificate of a dail server.
pub struct ServerIdentity {
    key: SigningKey,
    cert: CertificateDer<'static>,
    fingerprint: String,
}

impl std::fmt::Debug for ServerIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerIdentity")
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

fn serve_error(what: &str, path: &Path, err: impl std::fmt::Display) -> SyncError {
    SyncError::Serve(format!("{what} {}: {err}", path.display()))
}

impl ServerIdentity {
    /// Loads the key and certificate from `dir` (`ProfileDirs::serve_dir`), making whatever is
    /// missing: the key once, the certificate when it is missing or not the key's. `hosts` are
    /// the names and addresses a new certificate names besides `localhost` (`--advertise`).
    pub fn load_or_create(dir: &Path, hosts: &[String]) -> Result<Self, SyncError> {
        ensure_private_dir(dir)?;
        let key_path = dir.join(KEY_FILE);
        let key = match read_optional(&key_path)? {
            Some(bytes) => SigningKey::from_slice(&bytes)
                .map_err(|_| serve_error("invalid key in", &key_path, "not a P-256 scalar"))?,
            None => {
                let key = generate_key()?;
                write_private(&key_path, key.to_bytes().as_slice())?;
                tracing::info!(path = %key_path.display(), "dail server key created");
                key
            }
        };
        let cert_path = dir.join(CERT_FILE);
        let existing = read_optional(&cert_path)?.filter(|der| belongs_to(der, &key));
        let cert = match existing {
            Some(der) => der,
            None => {
                let der = build_certificate(&key, hosts, SystemTime::now())?;
                write_private(&cert_path, &der)?;
                tracing::info!(path = %cert_path.display(), "dail server certificate created");
                der
            }
        };
        let fingerprint = crate::trust::fingerprint(&cert);
        Ok(Self {
            key,
            cert: CertificateDer::from(cert),
            fingerprint,
        })
    }

    /// The SHA-256 fingerprint of the certificate, lower-case hex: what devices pin.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// The certificate, DER.
    pub fn certificate(&self) -> &[u8] {
        self.cert.as_ref()
    }

    /// The TLS server configuration: TLS 1.3 with the app-tls provider, this certificate, no
    /// client certificates (devices authenticate with a token inside the connection).
    pub(crate) fn server_config(&self) -> Result<Arc<ServerConfig>, SyncError> {
        let key = Arc::new(ServerKey(self.key.clone()));
        let certified = CertifiedKey::new(vec![self.cert.clone()], key);
        let resolver: Arc<dyn ResolvesServerCert> = Arc::new(SingleCertAndKey::from(certified));
        let config = ServerConfig::builder_with_provider(Arc::new(app_tls::provider()))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|err| SyncError::Tls(err.to_string()))?
            .with_no_client_auth()
            .with_cert_resolver(resolver);
        Ok(Arc::new(config))
    }
}

/// Creates `dir` (and its parents) with user-only permissions, or makes an existing one
/// user-only. On Windows the profile directory's own permissions apply, as for the rest of the
/// profile.
pub(crate) fn ensure_private_dir(dir: &Path) -> Result<(), SyncError> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(dir)
        .map_err(|err| serve_error("cannot create", dir, err))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|err| serve_error("cannot protect", dir, err))?;
    }
    Ok(())
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, SyncError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(serve_error("cannot read", path, err)),
    }
}

/// Writes `bytes` to `path` through a user-only temporary file and a rename.
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> Result<(), SyncError> {
    use std::io::Write as _;
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    let _ = std::fs::remove_file(&tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|err| serve_error("cannot write", &tmp, err))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|err| serve_error("cannot write", &tmp, err))?;
    drop(file);
    std::fs::rename(&tmp, path).map_err(|err| serve_error("cannot write", path, err))
}

/// A fresh P-256 key from the OS random source.
fn generate_key() -> Result<SigningKey, SyncError> {
    // A random 32-byte string is a valid scalar (non-zero, below the group order) with
    // probability 1 - 2^-32; try again in the unlikely other case.
    for _ in 0..8 {
        let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
        getrandom::fill(bytes.as_mut_slice())
            .map_err(|err| SyncError::Serve(format!("no OS randomness: {err}")))?;
        if let Ok(key) = SigningKey::from_slice(bytes.as_slice()) {
            return Ok(key);
        }
    }
    Err(SyncError::Serve(
        "could not generate a P-256 key".to_owned(),
    ))
}

/// Whether the certificate `der` holds the public key of `key`.
fn belongs_to(der: &[u8], key: &SigningKey) -> bool {
    let Ok(cert) = Certificate::from_der(der) else {
        return false;
    };
    let Ok(ours) = SubjectPublicKeyInfo::from_key(key.verifying_key()) else {
        return false;
    };
    cert.tbs_certificate().subject_public_key_info() == &ours
}

/// The names of a new certificate: `hosts` that are IP addresses or DNS names, then
/// `localhost`, `127.0.0.1` and `::1`. Anything else is left out (and logged).
fn alt_names(hosts: &[String]) -> Vec<GeneralName> {
    let mut names: Vec<GeneralName> = Vec::new();
    let mut push = |name: GeneralName| {
        if !names.contains(&name) {
            names.push(name);
        }
    };
    for host in hosts
        .iter()
        .map(String::as_str)
        .chain(["localhost", "127.0.0.1", "::1"])
    {
        let host = host.trim().trim_start_matches('[').trim_end_matches(']');
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            push(GeneralName::from(ip));
        } else if is_dns_name(host)
            && let Ok(name) = Ia5String::new(host)
        {
            push(GeneralName::DnsName(name));
        } else {
            tracing::warn!(
                host,
                "not a host name or IP address; left out of the certificate"
            );
        }
    }
    names
}

/// Whether `host` is a DNS name: labels of letters, digits and `-`, at most 253 characters.
fn is_dns_name(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

/// A self-signed end-entity certificate for TLS servers.
struct ServeProfile {
    subject: Name,
    names: Vec<GeneralName>,
}

impl BuilderProfile for ServeProfile {
    fn get_issuer(&self, subject: &Name) -> Name {
        subject.clone()
    }

    fn get_subject(&self) -> Name {
        self.subject.clone()
    }

    fn build_extensions(
        &self,
        spk: SubjectPublicKeyInfoRef<'_>,
        _issuer_spk: SubjectPublicKeyInfoRef<'_>,
        tbs: &TbsCertificate,
    ) -> x509_cert::builder::Result<Vec<Extension>> {
        let subject = tbs.subject();
        let mut extensions = Vec::new();
        let basic = BasicConstraints {
            ca: false,
            path_len_constraint: None,
        };
        extensions.push((&basic).to_extension(subject, &extensions)?);
        let usage = KeyUsage(KeyUsages::DigitalSignature.into());
        extensions.push((&usage).to_extension(subject, &extensions)?);
        let extended = ExtendedKeyUsage(vec![ID_KP_SERVER_AUTH]);
        extensions.push((&extended).to_extension(subject, &extensions)?);
        let names = SubjectAltName(self.names.clone());
        extensions.push((&names).to_extension(subject, &extensions)?);
        let key_id = SubjectKeyIdentifier::try_from(spk)?;
        extensions.push((&key_id).to_extension(subject, &extensions)?);
        Ok(extensions)
    }
}

/// Builds and signs the certificate of `key`, valid from `now` for [`CERT_VALIDITY`].
fn build_certificate(
    key: &SigningKey,
    hosts: &[String],
    now: SystemTime,
) -> Result<Vec<u8>, SyncError> {
    let failed = |err: &dyn std::fmt::Display| {
        SyncError::Serve(format!("cannot build the certificate: {err}"))
    };
    let subject = Name::from_str("CN=dail server").map_err(|err| failed(&err))?;
    let profile = ServeProfile {
        subject,
        names: alt_names(hosts),
    };
    let mut serial = [0u8; 16];
    getrandom::fill(&mut serial).map_err(|err| failed(&err))?;
    // Positive and 16 bytes long (RFC 5280 §4.1.2.2).
    serial[0] = (serial[0] & 0x7f) | 0x40;
    let serial = SerialNumber::new(&serial).map_err(|err| failed(&err))?;
    let not_before = Time::try_from(now).map_err(|err| failed(&err))?;
    let not_after = Time::try_from(now + CERT_VALIDITY).map_err(|err| failed(&err))?;
    let spki = SubjectPublicKeyInfo::from_key(key.verifying_key()).map_err(|err| failed(&err))?;
    let builder =
        CertificateBuilder::new(profile, serial, Validity::new(not_before, not_after), spki)
            .map_err(|err| failed(&err))?;
    let cert = builder
        .build::<_, DerSignature>(key)
        .map_err(|err| failed(&err))?;
    cert.to_der().map_err(|err| failed(&err))
}

/// The server's key as rustls signs with it: ECDSA P-256 with SHA-256 (RFC 6979 nonces).
#[derive(Debug)]
struct ServerKey(SigningKey);

impl rustls::sign::SigningKey for ServerKey {
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn rustls::sign::Signer>> {
        offered
            .contains(&SignatureScheme::ECDSA_NISTP256_SHA256)
            .then(|| Box::new(ServerSigner(self.0.clone())) as Box<dyn rustls::sign::Signer>)
    }

    fn algorithm(&self) -> SignatureAlgorithm {
        SignatureAlgorithm::ECDSA
    }
}

#[derive(Debug)]
struct ServerSigner(SigningKey);

impl rustls::sign::Signer for ServerSigner {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> {
        use p256::ecdsa::signature::Signer as _;
        let signature: DerSignature = self
            .0
            .try_sign(message)
            .map_err(|_| rustls::Error::General("signing failed".to_owned()))?;
        Ok(signature.as_bytes().to_vec())
    }

    fn scheme(&self) -> SignatureScheme {
        SignatureScheme::ECDSA_NISTP256_SHA256
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_names_are_checked() {
        assert!(is_dns_name("home.example.org"));
        assert!(is_dns_name("box-1"));
        assert!(!is_dns_name(""));
        assert!(!is_dns_name("-bad.example"));
        assert!(!is_dns_name("under_score"));
        assert!(!is_dns_name("a..b"));
    }

    #[test]
    fn alt_names_take_hosts_and_localhost_once() {
        let names = alt_names(&[
            "100.64.1.2".to_owned(),
            "home.tail1234.ts.net".to_owned(),
            "localhost".to_owned(),
            "not a host".to_owned(),
        ]);
        assert_eq!(names.len(), 5);
    }
}
