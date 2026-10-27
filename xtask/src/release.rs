//! `cargo xtask release …` (Wave 7, ADR 0011):
//!
//! - `versions`: prints every version site and the version it holds; fails if they differ.
//! - `check-tag <vX.Y.Z>`: fails unless every version site holds the tag's version (the guard job
//!   of `release.yml`).
//! - `next <patch|minor|major>`: prints the version after the current one (MISC-9).
//! - `bump <x.y.z> [--note <text>]`: writes the version into every site, refreshes `Cargo.lock` and
//!   the mobile contract fixtures (the About frame shows the version), and adds a `CHANGELOG.md`
//!   section: `- <text>`, or a stub to fill in by hand.
//! - `notes`: prints the `CHANGELOG.md` section of the current version (the release notes).
//! - `manifest`, `keygen`: see [`crate::manifest`].

use std::io::Write;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::workspace_root;

pub(crate) const USAGE: &str = "cargo xtask release versions|check-tag <vX.Y.Z>|next <patch|minor|major>|bump <x.y.z> [--note <text>]|notes|manifest [--dist DIR]|keygen <DIR>";

/// A file that spells out the release version: the version is the text between `prefix` and
/// `suffix` on the first line that starts with `prefix` (after leading spaces). The Android
/// `versionName` and `versionCode` are read from `Cargo.toml` by Gradle, so they are not a site.
pub(crate) struct Site {
    pub path: &'static str,
    pub prefix: &'static str,
    pub suffix: &'static str,
}

/// Every version site. `release_sites_agree` pins the table against the files.
pub(crate) const SITES: &[Site] = &[
    // `[workspace.package] version`: every crate takes it, and Gradle reads it.
    Site {
        path: "Cargo.toml",
        prefix: "version = \"",
        suffix: "\"",
    },
    Site {
        path: "ios/project.yml",
        prefix: "MARKETING_VERSION: \"",
        suffix: "\"",
    },
];

/// The version a site holds, and the byte range it occupies in `text`.
fn find(site: &Site, text: &str) -> Option<(String, std::ops::Range<usize>)> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix(site.prefix) {
            let end = rest.find(site.suffix)?;
            let start = offset + (line.len() - trimmed.len()) + site.prefix.len();
            return Some((rest[..end].to_owned(), start..start + end));
        }
        offset += line.len();
    }
    None
}

/// `(path, version)` for every site.
pub(crate) fn versions(root: &Path) -> Result<Vec<(&'static str, String)>> {
    SITES
        .iter()
        .map(|site| {
            let text = std::fs::read_to_string(root.join(site.path))
                .with_context(|| format!("reading {}", site.path))?;
            let (version, _) = find(site, &text).with_context(|| {
                format!("{} has no line starting with {}", site.path, site.prefix)
            })?;
            Ok((site.path, version))
        })
        .collect()
}

/// The one version every site holds.
pub(crate) fn current_version(root: &Path) -> Result<String> {
    let found = versions(root)?;
    let first = found
        .first()
        .map(|(_, v)| v.clone())
        .context("no version sites")?;
    if let Some((path, other)) = found.iter().find(|(_, v)| *v != first) {
        bail!(
            "version sites disagree: {} has {first}, {path} has {other}",
            SITES[0].path
        );
    }
    Ok(first)
}

/// `x.y.z` with numeric parts only (no pre-release: ADR 0011 has stable releases only).
pub(crate) fn is_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 4 && p.bytes().all(|b| b.is_ascii_digit()))
}

pub(crate) fn run(args: &[String]) -> Result<()> {
    let root = workspace_root()?;
    match args.first().map(String::as_str) {
        Some("versions") => {
            let mut out = std::io::stdout().lock();
            for (path, version) in versions(&root)? {
                writeln!(out, "{path}: {version}")?;
            }
            current_version(&root).map(|_| ())
        }
        Some("check-tag") => {
            let tag = args.get(1).context("usage: check-tag <vX.Y.Z>")?;
            let wanted = tag.strip_prefix('v').unwrap_or(tag);
            if !is_version(wanted) {
                bail!("tag {tag} is not vX.Y.Z");
            }
            let current = current_version(&root)?;
            if current != wanted {
                bail!("tag {tag} does not match the version sites ({current})");
            }
            Ok(())
        }
        Some("manifest") => crate::manifest::run(&root, &args[1..]),
        Some("keygen") => {
            crate::manifest::keygen(Path::new(args.get(1).context("usage: keygen <dir>")?))
        }
        Some("notes") => {
            let version = current_version(&root)?;
            let changelog = std::fs::read_to_string(root.join("CHANGELOG.md"))
                .context("reading CHANGELOG.md")?;
            let notes = notes(&changelog, &version)
                .with_context(|| format!("CHANGELOG.md has no section ## {version}"))?;
            let mut out = std::io::stdout().lock();
            writeln!(out, "{notes}")?;
            Ok(())
        }
        Some("next") => {
            let part = args.get(1).context("usage: next <patch|minor|major>")?;
            let next = next(&current_version(&root)?, part)?;
            let mut out = std::io::stdout().lock();
            writeln!(out, "{next}")?;
            Ok(())
        }
        Some("bump") => {
            let version = args.get(1).context("usage: bump <x.y.z> [--note <text>]")?;
            let note = match args.get(2).map(String::as_str) {
                None => None,
                Some("--note") => Some(args.get(3).context("--note needs a text")?.as_str()),
                Some(other) => bail!("unknown bump argument {other}"),
            };
            bump(&root, version, note)
        }
        _ => {
            let mut err = std::io::stderr().lock();
            writeln!(err, "usage: {USAGE}")?;
            bail!("unknown or missing release task")
        }
    }
}

/// The body of the `## <version>` section, trimmed; `None` if there is none or it is empty.
pub(crate) fn notes(changelog: &str, version: &str) -> Option<String> {
    let heading = format!("## {version}");
    let mut lines = changelog.lines().skip_while(|line| *line != heading);
    lines.next()?;
    let body: Vec<&str> = lines.take_while(|line| !line.starts_with("## ")).collect();
    let body = body.join("\n").trim().to_owned();
    (!body.is_empty() && body != "-").then_some(body)
}

/// The version after `current` (`x.y.z`): `patch` → `x.y.z+1`, `minor` → `x.y+1.0`, `major` →
/// `x+1.0.0`.
pub(crate) fn next(current: &str, part: &str) -> Result<String> {
    if !is_version(current) {
        bail!("{current} is not x.y.z");
    }
    let mut n = [0u32; 3];
    for (slot, text) in n.iter_mut().zip(current.split('.')) {
        *slot = text.parse()?;
    }
    let [major, minor, patch] = n;
    Ok(match part {
        "patch" => format!("{major}.{minor}.{}", patch + 1),
        "minor" => format!("{major}.{}.0", minor + 1),
        "major" => format!("{}.0.0", major + 1),
        other => bail!("{other} is not patch, minor or major"),
    })
}

/// One changelog line from `note`: whitespace runs (newlines included) become one space, so a
/// note can't add a heading of its own.
fn changelog_line(note: Option<&str>) -> String {
    let text = note.map_or_else(String::new, |n| {
        n.split_whitespace().collect::<Vec<_>>().join(" ")
    });
    format!("- {text}")
}

fn bump(root: &Path, version: &str, note: Option<&str>) -> Result<()> {
    if !is_version(version) {
        bail!("{version} is not x.y.z");
    }
    let current = current_version(root)?;
    for site in SITES {
        let path = root.join(site.path);
        let mut text = std::fs::read_to_string(&path)?;
        let (_, range) = find(site, &text).context("version site vanished")?;
        text.replace_range(range, version);
        std::fs::write(&path, text)?;
    }

    let changelog = root.join("CHANGELOG.md");
    let old = std::fs::read_to_string(&changelog).unwrap_or_default();
    let heading = format!("## {version}");
    if !old.lines().any(|line| line == heading) {
        let (head, rest) = match old.find("\n## ") {
            Some(at) => old.split_at(at + 1),
            None => (old.as_str(), ""),
        };
        let head = if head.is_empty() {
            "# Changelog\n\n"
        } else {
            head
        };
        let line = changelog_line(note);
        std::fs::write(&changelog, format!("{head}{heading}\n\n{line}\n\n{rest}"))?;
    }

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(&cargo)
        .current_dir(root)
        .args(["update", "--workspace", "--offline"])
        .status()?;
    if !status.success() {
        bail!("cargo update --workspace failed with {status}");
    }
    let status = Command::new(&cargo)
        .current_dir(root)
        .env("DAIL_UPDATE_CONTRACT", "1")
        .args(["test", "-p", "app-mobile", "--test", "it", "contract"])
        .status()?;
    if !status.success() {
        bail!("refreshing the contract fixtures failed with {status}");
    }
    let mut out = std::io::stdout().lock();
    writeln!(
        out,
        "bumped {current} → {version}; fill in CHANGELOG.md under {heading}"
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_sites_agree() {
        let root = workspace_root().unwrap();
        let found = versions(&root).unwrap();
        assert_eq!(found.len(), SITES.len());
        let version = current_version(&root).unwrap();
        assert!(is_version(&version), "{version}");
        // Cargo.toml's first `version = ` line is `[workspace.package]`'s.
        let cargo = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        let package = cargo.find("[workspace.package]").unwrap();
        assert!(cargo.find("version = \"").unwrap() > package);
    }

    #[test]
    fn find_replaces_only_the_version() {
        let site = Site {
            path: "x",
            prefix: "MARKETING_VERSION: \"",
            suffix: "\"",
        };
        let mut text = "a: 1\n    MARKETING_VERSION: \"0.1.0\"\n".to_owned();
        let (version, range) = find(&site, &text).unwrap();
        assert_eq!(version, "0.1.0");
        text.replace_range(range, "1.20.3");
        assert_eq!(text, "a: 1\n    MARKETING_VERSION: \"1.20.3\"\n");
    }

    #[test]
    fn notes_are_the_version_section() {
        let log = "# Changelog\n\n## 0.2.0\n\n- Updates.\n\n## 0.1.0\n\n- First.\n";
        assert_eq!(notes(log, "0.2.0").as_deref(), Some("- Updates."));
        assert_eq!(notes(log, "0.1.0").as_deref(), Some("- First."));
        assert_eq!(notes(log, "0.3.0"), None);
        assert_eq!(notes("## 0.3.0\n\n- \n", "0.3.0"), None);
        let root = workspace_root().unwrap();
        let changelog = std::fs::read_to_string(root.join("CHANGELOG.md")).unwrap();
        assert!(notes(&changelog, &current_version(&root).unwrap()).is_some());
    }

    #[test]
    fn next_increments_one_part() {
        assert_eq!(next("0.1.0", "patch").unwrap(), "0.1.1");
        assert_eq!(next("0.1.9", "patch").unwrap(), "0.1.10");
        assert_eq!(next("0.1.3", "minor").unwrap(), "0.2.0");
        assert_eq!(next("1.4.3", "major").unwrap(), "2.0.0");
        assert!(next("0.1.0", "skip").is_err());
        assert!(next("0.1", "patch").is_err());
    }

    #[test]
    fn changelog_line_is_one_line() {
        assert_eq!(changelog_line(None), "- ");
        assert_eq!(
            changelog_line(Some("Fix  the\n## 9.9.9\nrelease")),
            "- Fix the ## 9.9.9 release"
        );
    }

    #[test]
    fn versions_are_three_numbers() {
        assert!(is_version("0.1.0"));
        assert!(!is_version("0.1"));
        assert!(!is_version("0.1.0-rc1"));
        assert!(!is_version("v0.1.0"));
    }
}
