//! `cargo xtask ios …` (ADR 0010). There is no local Mac, so only `check` runs everywhere:
//!
//! - `check`: clippy (`-D warnings`) for [`TARGETS`] over [`CRATES`], with and without `demo`.
//!   Clippy type-checks and never links, so it needs the rustup targets but not Xcode. This is the
//!   local gate for the Rust side of the iOS app.
//! - `xcframework [--demo]` (macOS with Xcode only): `libdail.a` for both targets in release, then
//!   `xcodebuild -create-xcframework` with `crates/app-ios/include` as the headers, giving
//!   `ios/Frameworks/DailCore.xcframework` (or `DailCore-Demo.xcframework`, with the `demo` feature).
//! - `build` (macOS with Xcode and XcodeGen): both XCFrameworks, `xcodegen generate` over
//!   `ios/project.yml`, then `xcodebuild build` of the [`SCHEMES`] for the simulator.
//! - `test` (as `build`): `DailTests` (the contract fixtures and the demo host) in the scheme
//!   `DailDemo` on the newest available iPad simulator; the result bundle goes to [`RESULTS`].
//! - `app` (as `build`): each scheme archived for devices in Release, unsigned, and packed as
//!   `dist/dail-<version>[-demo]-unsigned.ipa` with a `.sha256` (plan §6: the user re-signs it).
//! - `measure` (macOS with Xcode): `app-io`'s `scale` measurements (redb and tantivy at 50k
//!   messages), built in release for `aarch64-apple-ios-sim` and run in the iPad simulator
//!   through `xcrun simctl spawn` (ROADMAP Wave 5 I3). `DailTests`' `ScaleTests` measure the app.
//! - `all`: `check`, `build`, `test` and `app`, the `ios` gate.
//!
//! A missing target, macOS or Xcode is reported as "cannot run", never as a pass (AGENTS.md §6).

use std::io::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{missing_targets, workspace_root};

pub const USAGE: &str = "cargo xtask ios check|xcframework [--demo]|build|test|app|measure|all";

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

/// The app targets of `ios/project.yml`, one scheme each: live and demo.
const SCHEMES: &[&str] = &["Dail", "DailDemo"];

/// `xcodebuild test`'s result bundle, which CI uploads when the tests fail.
const RESULTS: &str = "target/ios-tests.xcresult";

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
        ["build"] => build(),
        ["test"] => test(),
        ["app"] => app(),
        ["measure"] => measure(),
        ["all"] => check()
            .and_then(|()| build())
            .and_then(|()| test())
            .and_then(|()| app()),
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
    require_xcode("ios xcframework", &[])?;
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

/// Reports "cannot run" unless this is macOS with `xcodebuild` and every tool in `tools` (run with
/// `--version`).
fn require_xcode(task: &str, tools: &[&str]) -> Result<()> {
    let mut err = std::io::stderr().lock();
    if !cfg!(target_os = "macos") {
        writeln!(err, "{task}: cannot run: needs macOS with Xcode")?;
        bail!("{task} cannot run");
    }
    let found = |tool: &str, flag: &str| {
        Command::new(tool)
            .arg(flag)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    if !found("xcodebuild", "-version") {
        writeln!(err, "{task}: cannot run: `xcodebuild` not found")?;
        bail!("{task} cannot run");
    }
    if let Some(tool) = tools.iter().find(|tool| !found(tool, "--version")) {
        writeln!(err, "{task}: cannot run: `{tool}` not found")?;
        bail!("{task} cannot run");
    }
    Ok(())
}

/// Runs `program` in `dir`, failing on a non-zero exit.
fn run_in(dir: &Path, program: &str, args: &[&str]) -> Result<()> {
    writeln!(
        std::io::stderr().lock(),
        "ios: {program} {}",
        args.join(" ")
    )?;
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("failed to spawn `{program}`"))?;
    if !status.success() {
        bail!("`{program} {}` failed with {status}", args.join(" "));
    }
    Ok(())
}

/// Both XCFrameworks and the Xcode project generated from `ios/project.yml`.
fn generate(task: &str) -> Result<std::path::PathBuf> {
    let root = workspace_root()?;
    require_xcode(task, &["xcodegen"])?;
    xcframework(false)?;
    xcframework(true)?;
    let ios = root.join("ios");
    run_in(&ios, "xcodegen", &["generate", "--spec", "project.yml"])?;
    Ok(ios)
}

/// The app's version for `xcodebuild`: `[workspace.package] version`, the only place it is
/// written (`ios/project.yml` has none).
fn marketing() -> String {
    format!("MARKETING_VERSION={}", env!("CARGO_PKG_VERSION"))
}

/// Both XCFrameworks, the generated Xcode project, and every scheme built for the simulator.
fn build() -> Result<()> {
    let ios = generate("ios build")?;
    let marketing = marketing();
    for scheme in SCHEMES {
        run_in(
            &ios,
            "xcodebuild",
            &[
                "build",
                "-project",
                "Dail.xcodeproj",
                "-scheme",
                scheme,
                "-destination",
                "generic/platform=iOS Simulator",
                "-derivedDataPath",
                "../target/ios-xcode",
                &marketing,
            ],
        )?;
    }
    writeln!(std::io::stderr().lock(), "ios build passed")?;
    Ok(())
}

/// The UDID of the newest available iPad simulator (`xcrun simctl list -j devices available`).
fn ipad_simulator() -> Result<String> {
    let output = Command::new("xcrun")
        .args(["simctl", "list", "-j", "devices", "available"])
        .output()
        .context("failed to spawn `xcrun simctl`")?;
    if !output.status.success() {
        bail!("`xcrun simctl list` failed with {}", output.status);
    }
    let list: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("`xcrun simctl list` gave no JSON")?;
    // Runtimes are keyed `com.apple.CoreSimulator.SimRuntime.iOS-18-2`; the newest has the highest
    // version.
    let mut best: Option<(Vec<u32>, String)> = None;
    for (runtime, devices) in list["devices"].as_object().into_iter().flatten() {
        let Some(version) = runtime.rsplit_once(".iOS-").map(|(_, v)| v) else {
            continue;
        };
        let version: Vec<u32> = version.split('-').filter_map(|p| p.parse().ok()).collect();
        let ipad = devices.as_array().into_iter().flatten().find(|device| {
            device["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("iPad"))
        });
        if let Some(udid) = ipad.and_then(|device| device["udid"].as_str())
            && best.as_ref().is_none_or(|(newest, _)| version > *newest)
        {
            best = Some((version, udid.to_owned()));
        }
    }
    best.map(|(_, udid)| udid)
        .context("no available iPad simulator (`xcrun simctl list devices available`)")
}

/// `DailTests` in the `DailDemo` scheme on an iPad simulator.
fn test() -> Result<()> {
    let ios = generate("ios test")?;
    let root = workspace_root()?;
    let device = ipad_simulator()?;
    let results = root.join(RESULTS);
    // xcodebuild refuses to overwrite a result bundle.
    if results.exists() {
        std::fs::remove_dir_all(&results)
            .with_context(|| format!("failed to remove {}", results.display()))?;
    }
    let destination = format!("platform=iOS Simulator,id={device}");
    let results = results.display().to_string();
    let marketing = marketing();
    run_in(
        &ios,
        "xcodebuild",
        &[
            "test",
            "-project",
            "Dail.xcodeproj",
            "-scheme",
            "DailDemo",
            "-destination",
            &destination,
            "-derivedDataPath",
            "../target/ios-xcode",
            "-resultBundlePath",
            &results,
            &marketing,
        ],
    )?;
    writeln!(std::io::stderr().lock(), "ios test passed")?;
    Ok(())
}

/// Every scheme archived unsigned for devices and packed as an IPA in `dist/`, with its `.sha256`.
fn app() -> Result<()> {
    let ios = generate("ios app")?;
    let root = workspace_root()?;
    let version = env!("CARGO_PKG_VERSION");
    let marketing = marketing();
    let dist = root.join("dist");
    std::fs::create_dir_all(&dist)
        .with_context(|| format!("failed to create {}", dist.display()))?;
    for (scheme, product, suffix) in [("Dail", "Dail", ""), ("DailDemo", "DailDemo", "-demo")] {
        let archive = root.join(format!("target/ios-archive/{scheme}.xcarchive"));
        if archive.exists() {
            std::fs::remove_dir_all(&archive)
                .with_context(|| format!("failed to remove {}", archive.display()))?;
        }
        let archive_path = archive.display().to_string();
        run_in(
            &ios,
            "xcodebuild",
            &[
                "archive",
                "-project",
                "Dail.xcodeproj",
                "-scheme",
                scheme,
                "-configuration",
                "Release",
                "-destination",
                "generic/platform=iOS",
                "-derivedDataPath",
                "../target/ios-xcode",
                "-archivePath",
                &archive_path,
                &marketing,
                "CODE_SIGNING_ALLOWED=NO",
            ],
        )?;
        // An IPA is a zip of `Payload/<App>.app`.
        let staging = root.join(format!("target/ios-archive/{scheme}-ipa"));
        if staging.exists() {
            std::fs::remove_dir_all(&staging)
                .with_context(|| format!("failed to remove {}", staging.display()))?;
        }
        let payload = staging.join("Payload");
        std::fs::create_dir_all(&payload)
            .with_context(|| format!("failed to create {}", payload.display()))?;
        let app = archive.join(format!("Products/Applications/{product}.app"));
        run_in(
            &staging,
            "ditto",
            &[
                &app.display().to_string(),
                &payload.join(format!("{product}.app")).display().to_string(),
            ],
        )?;
        let name = format!("dail-{version}{suffix}-unsigned.ipa");
        let ipa = dist.join(&name);
        if ipa.exists() {
            std::fs::remove_file(&ipa)
                .with_context(|| format!("failed to remove {}", ipa.display()))?;
        }
        run_in(
            &staging,
            "zip",
            &["-qry", &ipa.display().to_string(), "Payload"],
        )?;
        let output = Command::new("shasum")
            .args(["-a", "256", &name])
            .current_dir(&dist)
            .output()
            .context("failed to spawn `shasum`")?;
        if !output.status.success() {
            bail!("`shasum -a 256 {name}` failed with {}", output.status);
        }
        let sha = dist.join(format!("{name}.sha256"));
        std::fs::write(&sha, &output.stdout)
            .with_context(|| format!("failed to write {}", sha.display()))?;
        writeln!(std::io::stderr().lock(), "ios: wrote {}", ipa.display())?;
    }
    Ok(())
}

/// `app-io`'s scale tests in the simulator: the test binary is built for the simulator target and
/// spawned in a booted iPad simulator, which runs it as an iOS process (mmap, file locks and the
/// sandbox's temporary directory as on the iPad).
fn measure() -> Result<()> {
    let root = workspace_root()?;
    require_xcode("ios measure", &[])?;
    require_targets(&root)?;
    let target = "aarch64-apple-ios-sim";
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let args = [
        "test",
        "--release",
        "--locked",
        "-p",
        "app-io",
        "--test",
        "it",
        "--target",
        target,
        "--no-run",
        "--message-format=json",
    ];
    writeln!(std::io::stderr().lock(), "ios: cargo {}", args.join(" "))?;
    let output = Command::new(&cargo)
        .args(args)
        .current_dir(&root)
        .stderr(std::process::Stdio::inherit())
        .output()
        .context("failed to spawn cargo")?;
    if !output.status.success() {
        bail!(
            "building the app-io tests for {target} failed with {}",
            output.status
        );
    }
    let binary = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| {
            message["reason"] == "compiler-artifact" && message["profile"]["test"] == true
        })
        .find_map(|message| message["executable"].as_str().map(str::to_owned))
        .context("cargo built no test executable")?;
    let device = ipad_simulator()?;
    // Booting a booted device fails; that is fine.
    let _ = Command::new("xcrun")
        .args(["simctl", "boot", &device])
        .status();
    run_in(&root, "xcrun", &["simctl", "bootstatus", &device, "-b"])?;
    run_in(
        &root,
        "xcrun",
        &[
            "simctl",
            "spawn",
            &device,
            &binary,
            "scale",
            "--ignored",
            "--nocapture",
            "--test-threads",
            "1",
        ],
    )?;
    writeln!(std::io::stderr().lock(), "ios measure passed")?;
    Ok(())
}
