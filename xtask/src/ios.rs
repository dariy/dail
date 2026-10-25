//! `cargo xtask ios …` (ADR 0010). There is no local Mac, so only `check` runs everywhere:
//!
//! - `check`: clippy (`-D warnings`) for [`TARGETS`] over [`CRATES`], with and without `demo`.
//!   Clippy type-checks and never links, so it needs the rustup targets but not Xcode. This is the
//!   local gate for the Rust side of the iOS app.
//! - `xcframework [--demo]` (macOS with Xcode only): `libdail.a` for both targets in release, then
//!   `xcodebuild -create-xcframework` with `crates/app-ios/include` as the headers, giving
//!   `ios/Frameworks/DailCore.xcframework` (or `DailCore-Demo.xcframework`, with the `demo` feature).
//!
//! A missing target, macOS or Xcode is reported as "cannot run", never as a pass (AGENTS.md §6).

use std::io::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{missing_targets, workspace_root};

pub const USAGE: &str = "cargo xtask ios check|xcframework [--demo]";

/// The device and the (arm64) simulator. Current runners and Macs are arm64, so there is no
/// `x86_64-apple-ios`.
const TARGETS: &[&str] = &["aarch64-apple-ios", "aarch64-apple-ios-sim"];

/// The crates the iOS app links (ADR 0010).
const CRATES: &[&str] = &[
    "app-core",
    "app-io",
    "app-sync",
    "app-tls",
    "app-front",
    "app-theme",
    "app-mobile",
    "app-host",
    "app-ios",
];

/// Where the XCFrameworks go; gitignored, and what `ios/project.yml` links.
const FRAMEWORKS_DIR: &str = "ios/Frameworks";

pub fn run(args: &[String]) -> Result<()> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["check"] => check(),
        ["xcframework"] => xcframework(false),
        ["xcframework", "--demo"] => xcframework(true),
        _ => {
            writeln!(std::io::stderr().lock(), "usage: {USAGE}")?;
            bail!("unknown or missing ios task")
        }
    }
}

fn cargo(root: &Path, args: &[&str]) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    writeln!(std::io::stderr().lock(), "ios: cargo {}", args.join(" "))?;
    let status = Command::new(&cargo)
        .args(args)
        .current_dir(root)
        .status()
        .with_context(|| format!("failed to spawn `cargo {}`", args.join(" ")))?;
    if !status.success() {
        bail!("`cargo {}` failed with {status}", args.join(" "));
    }
    Ok(())
}

fn require_targets(root: &Path) -> Result<()> {
    let missing = missing_targets(root, TARGETS)?;
    if !missing.is_empty() {
        writeln!(
            std::io::stderr().lock(),
            "ios: cannot run: Rust targets not installed: {} (rustup target add {})",
            missing.join(", "),
            missing.join(" ")
        )?;
        bail!("ios cannot run");
    }
    Ok(())
}

fn check() -> Result<()> {
    let root = workspace_root()?;
    require_targets(&root)?;
    for target in TARGETS {
        for demo in [false, true] {
            let mut args = vec!["clippy", "--target", target, "--all-targets"];
            for name in CRATES {
                args.extend(["-p", name]);
            }
            if demo {
                args.extend(["--features", "app-ios/demo"]);
            }
            args.extend(["--", "-D", "warnings"]);
            cargo(&root, &args)?;
        }
    }
    writeln!(std::io::stderr().lock(), "ios check passed")?;
    Ok(())
}

fn xcframework(demo: bool) -> Result<()> {
    let root = workspace_root()?;
    let mut err = std::io::stderr().lock();
    if !cfg!(target_os = "macos") {
        writeln!(err, "ios xcframework: cannot run: needs macOS with Xcode")?;
        bail!("ios xcframework cannot run");
    }
    let xcodebuild = Command::new("xcodebuild").arg("-version").output();
    if !xcodebuild.is_ok_and(|output| output.status.success()) {
        writeln!(err, "ios xcframework: cannot run: `xcodebuild` not found")?;
        bail!("ios xcframework cannot run");
    }
    drop(err);
    require_targets(&root)?;
    // A target dir per flavour, so the two `libdail.a` builds never overwrite each other.
    let (target_dir, name) = if demo {
        ("target/ios-demo", "DailCore-Demo.xcframework")
    } else {
        ("target/ios-live", "DailCore.xcframework")
    };
    let mut xcode = vec!["-create-xcframework".to_owned()];
    for target in TARGETS {
        let mut args = vec![
            "build",
            "--release",
            "--locked",
            "-p",
            "app-ios",
            "--target",
            target,
            "--target-dir",
            target_dir,
        ];
        if demo {
            args.extend(["--features", "demo"]);
        }
        cargo(&root, &args)?;
        let library = root.join(target_dir).join(target).join("release/libdail.a");
        xcode.extend([
            "-library".to_owned(),
            library.display().to_string(),
            "-headers".to_owned(),
            root.join("crates/app-ios/include").display().to_string(),
        ]);
    }
    let output = root.join(FRAMEWORKS_DIR).join(name);
    // xcodebuild refuses to overwrite an existing framework.
    if output.exists() {
        std::fs::remove_dir_all(&output)
            .with_context(|| format!("failed to remove {}", output.display()))?;
    }
    xcode.extend(["-output".to_owned(), output.display().to_string()]);
    writeln!(
        std::io::stderr().lock(),
        "ios: xcodebuild {}",
        xcode.join(" ")
    )?;
    let status = Command::new("xcodebuild")
        .args(&xcode)
        .status()
        .context("failed to spawn `xcodebuild`")?;
    if !status.success() {
        bail!("xcodebuild -create-xcframework failed with {status}");
    }
    writeln!(std::io::stderr().lock(), "ios: wrote {}", output.display())?;
    Ok(())
}
