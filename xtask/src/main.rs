//! `cargo xtask verify [GATE…]` — runs the Verification Loop gates from `AGENTS.md §6` in order and
//! stops at the first failure. Naming gates runs only those, still in gate order; CI runs its jobs
//! this way. The `android` gate runs only when named (it needs the Android Rust targets).
//!
//! `cargo xtask android check` — clippy (`-D warnings`) for the Android targets over the crates the
//! app links (ADR 0009).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// One gate of the Verification Loop.
struct Gate {
    name: &'static str,
    args: &'static [&'static str],
    /// Whether a plain `cargo xtask verify` runs it.
    default: bool,
}

/// `fmt` and `deny` compile nothing and take seconds, so they run before the gates that compile.
/// There is no separate `cargo check` gate: clippy runs the same compiler analysis and also fails on
/// rustc warnings, so a check before it only type-checked the workspace twice.
const GATES: &[Gate] = &[
    Gate {
        name: "fmt",
        args: &["fmt", "--all", "--check"],
        default: true,
    },
    Gate {
        name: "deny",
        args: &["deny", "check"],
        default: true,
    },
    Gate {
        name: "clippy",
        args: &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        default: true,
    },
    Gate {
        name: "test",
        args: &["test", "--workspace"],
        default: true,
    },
    Gate {
        name: "smoke",
        args: &[
            "run",
            "--quiet",
            "-p",
            "app-cli",
            "--",
            "batch",
            "--backend",
            "memory",
            "tests/smoke/basic.jsonl",
        ],
        default: true,
    },
    Gate {
        name: "android",
        args: &["xtask", "android", "check"],
        default: false,
    },
];

/// The Android ABIs' Rust targets (arm64-v8a, x86_64).
const ANDROID_TARGETS: &[&str] = &["aarch64-linux-android", "x86_64-linux-android"];

/// The crates the Android app links (ADR 0009). `app-mobile` and `app-android` join them in A1.
const ANDROID_CRATES: &[&str] = &[
    "app-core",
    "app-io",
    "app-sync",
    "app-tls",
    "app-front",
    "app-theme",
];

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("verify") => verify(&args.collect::<Vec<_>>()),
        Some("android") => match args.next().as_deref() {
            Some("check") => android_check(),
            _ => {
                let mut err = std::io::stderr().lock();
                writeln!(err, "usage: cargo xtask android check")?;
                bail!("unknown or missing android task")
            }
        },
        _ => {
            let mut err = std::io::stderr().lock();
            writeln!(err, "usage: cargo xtask verify [{}]…", gate_names("|"))?;
            writeln!(err, "       cargo xtask android check")?;
            bail!("unknown or missing task")
        }
    }
}

fn gate_names(separator: &str) -> String {
    GATES
        .iter()
        .map(|gate| gate.name)
        .collect::<Vec<_>>()
        .join(separator)
}

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .context("xtask must live one level below the workspace root")
}

/// Runs the gates named in `only` (all of them if it is empty).
fn verify(only: &[String]) -> Result<()> {
    if let Some(unknown) = only
        .iter()
        .find(|name| !GATES.iter().any(|gate| gate.name == name.as_str()))
    {
        bail!(
            "unknown gate `{unknown}`; the gates are {}",
            gate_names(", ")
        );
    }
    let root = workspace_root()?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut err = std::io::stderr().lock();
    let mut passed = Vec::new();
    for (index, gate) in GATES.iter().enumerate() {
        let named = only.iter().any(|name| name == gate.name);
        if (only.is_empty() && !gate.default) || (!only.is_empty() && !named) {
            continue;
        }
        let number = index + 1;
        writeln!(
            err,
            "gate {number} ({}): cargo {}",
            gate.name,
            gate.args.join(" ")
        )?;
        let status = Command::new(&cargo)
            .args(gate.args)
            .current_dir(&root)
            .status()
            .with_context(|| format!("failed to spawn `cargo {}`", gate.args.join(" ")))?;
        if !status.success() {
            bail!("gate {number} ({}) failed with {status}", gate.name);
        }
        passed.push(gate.name);
    }
    if only.is_empty() {
        writeln!(err, "all gates passed")?;
    } else {
        writeln!(err, "gates passed: {}", passed.join(", "))?;
    }
    Ok(())
}

/// Clippy for every Android target over [`ANDROID_CRATES`]. Clippy type-checks and never links, so
/// it needs the rustup targets but not the NDK. A missing target is reported as "cannot run".
fn android_check() -> Result<()> {
    let root = workspace_root()?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut err = std::io::stderr().lock();
    let missing = missing_targets(&root)?;
    if !missing.is_empty() {
        writeln!(
            err,
            "android: cannot run: Rust targets not installed: {} (rustup target add {})",
            missing.join(", "),
            missing.join(" ")
        )?;
        bail!("android check cannot run");
    }
    for target in ANDROID_TARGETS {
        let mut args = vec!["clippy", "--target", target, "--all-targets"];
        for name in ANDROID_CRATES {
            args.extend(["-p", name]);
        }
        args.extend(["--", "-D", "warnings"]);
        writeln!(err, "android ({target}): cargo {}", args.join(" "))?;
        let status = Command::new(&cargo)
            .args(&args)
            .current_dir(&root)
            .status()
            .with_context(|| format!("failed to spawn `cargo {}`", args.join(" ")))?;
        if !status.success() {
            bail!("android check for {target} failed with {status}");
        }
    }
    Ok(())
}

/// The Android targets whose standard library is not installed for the active toolchain.
fn missing_targets(root: &Path) -> Result<Vec<&'static str>> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let mut missing = Vec::new();
    for target in ANDROID_TARGETS {
        let output = Command::new(&rustc)
            .args(["--print", "target-libdir", "--target", target])
            .current_dir(root)
            .output()
            .with_context(|| format!("failed to spawn `{rustc}`"))?;
        let dir = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !output.status.success() || !has_std(&dir) {
            missing.push(*target);
        }
    }
    Ok(missing)
}

/// Whether `dir` (a target libdir) holds the standard library.
fn has_std(dir: &str) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with("libstd-"))
    })
}
