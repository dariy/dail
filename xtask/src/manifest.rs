//! `cargo xtask release manifest [--dist DIR]` and `cargo xtask release keygen DIR` (ADR 0011).
//!
//! `manifest` writes `DIR/manifest.json` (default `dist/`) from the release artifacts found there,
//! with each one's sha256 and size, and signs it as `DIR/manifest.json.minisig`. The secret key is
//! the text of a minisign secret key file in `DAIL_UPDATE_SIGNING_KEY` and its password in
//! `DAIL_UPDATE_SIGNING_PASSWORD` (empty for an unencrypted key); neither is written anywhere. The
//! trusted comment is `version:<x.y.z>`, which dail checks against the manifest.
//!
//! `keygen` writes a new encrypted key pair (`dail-update.key`, `dail-update.pub`) into `DIR`,
//! with the password from `DAIL_UPDATE_SIGNING_PASSWORD`. Key handling: `packaging/README.md`.

use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::release::current_version;

/// Where the release assets live once `gh release create` has uploaded them.
const DOWNLOAD_BASE: &str = "https://github.com/dariy/dail/releases/download";
const RELEASE_PAGE_BASE: &str = "https://github.com/dariy/dail/releases/tag";

/// The `(target, kind)` of a release asset, from its file name; `None` for files that are not
/// installable (archives, checksums, the demo IPA).
pub(crate) fn classify(file: &str, version: &str) -> Option<(String, String)> {
    let linux = "x86_64-unknown-linux-gnu";
    let windows = "x86_64-pc-windows-msvc";
    if file == format!("dail_{version}-1_amd64.deb") {
        return Some((linux.into(), "deb".into()));
    }
    let rest = file.strip_prefix(&format!("dail-{version}-"))?;
    let found = match rest {
        "x86_64-unknown-linux-gnu" => (linux, "exe".to_owned()),
        "x86_64-pc-windows-msvc.exe" => (windows, "exe".to_owned()),
        "x86_64.AppImage" => (linux, "appimage".to_owned()),
        "x86_64.msi" => (windows, "msi".to_owned()),
        "unsigned.ipa" => ("ios", "ipa".to_owned()),
        _ => {
            let abi = rest.strip_suffix(".apk")?;
            if abi.is_empty() || abi.contains('.') {
                return None;
            }
            ("android", format!("apk-{abi}"))
        }
    };
    Some((found.0.to_owned(), found.1))
}

fn sha256_and_size(path: &Path) -> Result<(String, u64)> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    let hex = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((hex, size))
}

/// The manifest for the artifacts in `dist`.
pub(crate) fn build(dist: &Path, version: &str, published_ms: i64) -> Result<Value> {
    let mut names: Vec<String> = std::fs::read_dir(dist)
        .with_context(|| format!("reading {}", dist.display()))?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect();
    names.sort();
    let mut artifacts = Vec::new();
    for name in names {
        let Some((target, kind)) = classify(&name, version) else {
            continue;
        };
        let (sha256, size) = sha256_and_size(&dist.join(&name))?;
        artifacts.push(json!({
            "target": target,
            "kind": kind,
            "url": format!("{DOWNLOAD_BASE}/v{version}/{name}"),
            "sha256": sha256,
            "size": size,
        }));
    }
    if artifacts.is_empty() {
        bail!("no release artifacts in {}", dist.display());
    }
    Ok(json!({
        "version": version,
        "published_ms": published_ms,
        "notes_url": format!("{RELEASE_PAGE_BASE}/v{version}"),
        "artifacts": artifacts,
    }))
}

fn password() -> Option<String> {
    std::env::var("DAIL_UPDATE_SIGNING_PASSWORD")
        .ok()
        .filter(|p| !p.is_empty())
}

/// The minisign signature of `data`, with the trusted comment `version:<version>`.
pub(crate) fn sign(
    secret_box: &str,
    password: Option<String>,
    data: &[u8],
    version: &str,
) -> Result<String> {
    let sk_box = minisign::SecretKeyBox::from_string(secret_box)
        .map_err(|e| anyhow::anyhow!("the signing key does not parse: {e}"))?;
    let sk = match password {
        Some(password) => sk_box.into_secret_key(Some(password)),
        None => sk_box.into_unencrypted_secret_key(),
    }
    .map_err(|e| anyhow::anyhow!("the signing key does not open: {e}"))?;
    let signature = minisign::sign(
        None,
        &sk,
        data,
        Some(&format!("version:{version}")),
        Some("dail update manifest"),
    )
    .map_err(|e| anyhow::anyhow!("signing failed: {e}"))?;
    Ok(signature.to_string())
}

pub(crate) fn run(root: &Path, args: &[String]) -> Result<()> {
    let mut dist = root.join("dist");
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--dist" => dist = rest.next().context("--dist needs a directory")?.into(),
            other => bail!("unknown argument {other}"),
        }
    }
    let version = current_version(root)?;
    // SOURCE_DATE_EPOCH (seconds) makes the manifest reproducible; otherwise now.
    let published_ms = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(secs) => secs.trim().parse::<i64>().context("SOURCE_DATE_EPOCH")? * 1000,
        Err(_) => i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis(),
        )?,
    };
    let manifest = build(&dist, &version, published_ms)?;
    let mut text = serde_json::to_string_pretty(&manifest)?;
    text.push('\n');
    let secret = std::env::var("DAIL_UPDATE_SIGNING_KEY")
        .context("DAIL_UPDATE_SIGNING_KEY (the minisign secret key) is not set")?;
    let signature = sign(&secret, password(), text.as_bytes(), &version)?;
    std::fs::write(dist.join("manifest.json"), &text)?;
    std::fs::write(dist.join("manifest.json.minisig"), signature)?;
    let count = manifest["artifacts"].as_array().map_or(0, Vec::len);
    let mut out = std::io::stdout().lock();
    writeln!(
        out,
        "wrote {}/manifest.json ({count} artifacts) and its signature",
        dist.display()
    )?;
    Ok(())
}

pub(crate) fn keygen(dir: &Path) -> Result<()> {
    let password = password().context("set DAIL_UPDATE_SIGNING_PASSWORD to encrypt the new key")?;
    let pair = minisign::KeyPair::generate_encrypted_keypair(Some(password))
        .map_err(|e| anyhow::anyhow!("key generation failed: {e}"))?;
    let secret = pair
        .sk
        .to_box(Some("dail update signing key"))
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .to_string();
    let public = pair
        .pk
        .to_box()
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .to_string();
    std::fs::create_dir_all(dir)?;
    let key = dir.join("dail-update.key");
    if key.exists() {
        bail!("{} exists; refusing to overwrite a key", key.display());
    }
    std::fs::write(&key, secret)?;
    std::fs::write(dir.join("dail-update.pub"), &public)?;
    let mut out = std::io::stdout().lock();
    writeln!(
        out,
        "wrote {} and dail-update.pub; the public key is the last line of the .pub",
        key.display()
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_maps_every_release_file() {
        let v = "1.2.3";
        let cases = [
            (
                "dail_1.2.3-1_amd64.deb",
                Some(("x86_64-unknown-linux-gnu", "deb")),
            ),
            (
                "dail-1.2.3-x86_64-unknown-linux-gnu",
                Some(("x86_64-unknown-linux-gnu", "exe")),
            ),
            (
                "dail-1.2.3-x86_64-pc-windows-msvc.exe",
                Some(("x86_64-pc-windows-msvc", "exe")),
            ),
            (
                "dail-1.2.3-x86_64.AppImage",
                Some(("x86_64-unknown-linux-gnu", "appimage")),
            ),
            (
                "dail-1.2.3-x86_64.msi",
                Some(("x86_64-pc-windows-msvc", "msi")),
            ),
            (
                "dail-1.2.3-arm64-v8a.apk",
                Some(("android", "apk-arm64-v8a")),
            ),
            ("dail-1.2.3-unsigned.ipa", Some(("ios", "ipa"))),
            ("dail-1.2.3-demo-unsigned.ipa", None),
            ("dail-1.2.3-x86_64.msi.sha256", None),
            ("dail-1.2.3-x86_64-unknown-linux-gnu.tar.gz", None),
            ("dail-1.2.2-x86_64.msi", None),
        ];
        for (file, want) in cases {
            let got = classify(file, v);
            let got = got.as_ref().map(|(t, k)| (t.as_str(), k.as_str()));
            assert_eq!(got, want, "{file}");
        }
    }

    #[test]
    fn the_manifest_is_signed_with_the_version() {
        let dir = std::env::temp_dir().join(format!("dail-xtask-manifest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("dail-1.2.3-x86_64.msi"), b"msi").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let manifest = build(&dir, "1.2.3", 5).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let artifacts = manifest["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0]["size"], 3);
        assert_eq!(artifacts[0]["sha256"], sha_hex(b"msi"));
        assert_eq!(
            artifacts[0]["url"],
            "https://github.com/dariy/dail/releases/download/v1.2.3/dail-1.2.3-x86_64.msi"
        );

        let pair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let secret = pair.sk.to_box(None).unwrap().to_string();
        let text = serde_json::to_string(&manifest).unwrap();
        let signature = sign(&secret, None, text.as_bytes(), "1.2.3").unwrap();
        let sig_box = minisign::SignatureBox::from_string(&signature).unwrap();
        assert_eq!(sig_box.trusted_comment().unwrap(), "version:1.2.3");
        minisign::verify(
            &pair.pk,
            &sig_box,
            std::io::Cursor::new(text.as_bytes()),
            true,
            false,
            false,
        )
        .unwrap();
    }

    fn sha_hex(data: &[u8]) -> String {
        Sha256::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}
