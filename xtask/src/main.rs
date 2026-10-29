//! `cargo xtask verify [GATE…]` — runs the Verification Loop gates from `AGENTS.md §6` in order and
//! stops at the first failure. Naming gates runs only those, still in gate order; CI runs its jobs
//! this way. The `android` and `ios` gates run only when named (they need the Android or iOS Rust
//! targets, and `ios` beyond `check` needs macOS with Xcode).
//!
//! `cargo xtask android check` — clippy (`-D warnings`) for the Android targets over the crates the
//! app links, then Gradle's formatter, licence allowlist, Android Lint and JVM tests (ADR 0009).
//!
//! `cargo xtask android apk` — assembles the APKs [`APK_TASKS`] names (cargo-ndk builds `libdail.so`).
//!
//! `cargo xtask android apk-check` — checks the APKs under `android/app/build/outputs/apk`: nothing
//! but `libdail.so` (or an allowlisted prebuilt) under `lib/`, 16 KB alignment, the permissions
//! pinned, release builds not debuggable and without the debug bridge.
//!
//! `cargo xtask android licences [--check]` — writes [`LICENCE_LISTS`]: the Rust crates
//! `libdail.so` links and the Android libraries the release APK carries, with their licences, which
//! the About screen shows. `--check` (run by `check`) fails when a list is stale.
//!
//! `cargo xtask android all` — `check`, `apk` and `apk-check`: the `android` gate.
//!
//! `cargo xtask ios check|xcframework [--demo]|build|test|app|all` — see [`ios`] (ADR 0010).
//!
//! `cargo xtask release versions|check-tag|bump|manifest|keygen` — see [`release`] (Wave 7, ADR 0011).
//!
//! `cargo xtask android test [ARGS…]` — builds the test binaries of [`ANDROID_TEST_CRATES`] for
//! `x86_64-linux-android` with cargo-ndk, pushes them and their test data to the device adb sees
//! (an emulator) and runs them there; `ARGS` go to every test binary.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

mod ios;
mod manifest;
mod release;

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
    // The patched crates under vendor/ are not members, so `--workspace` skips their tests (ADR 0012).
    Gate {
        name: "vendor",
        args: &["test", "-p", "hayro-postscript", "--lib"],
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
        args: &["xtask", "android", "all"],
        default: false,
    },
    Gate {
        name: "ios",
        args: &["xtask", "ios", "all"],
        default: false,
    },
];

/// The Android ABIs' Rust targets (arm64-v8a, x86_64).
const ANDROID_TARGETS: &[&str] = &["aarch64-linux-android", "x86_64-linux-android"];

/// The crates the Android app links (ADR 0009).
const ANDROID_CRATES: &[&str] = &[
    "app-core",
    "app-io",
    "app-sync",
    "app-tls",
    "app-front",
    "app-theme",
    "app-mobile",
    "app-host",
    "app-android",
];

/// The crates whose tests `cargo xtask android test` runs on the device: the ones that meet bionic
/// (redb, tantivy's mmap, getrandom, tokio, TLS). `app-mobile` joins them in A1.
const ANDROID_TEST_CRATES: &[&str] = &["app-core", "app-io", "app-tls", "app-sync"];

/// The Android target and API level (minSdk, ADR 0009) of the on-device tests.
const ANDROID_TEST_TARGET: &str = "x86_64-linux-android";
const ANDROID_TEST_API: &str = "29";

/// Where the tests live on the device; it mirrors the workspace, so snapshots resolve.
const DEVICE_ROOT: &str = "/data/local/tmp/dail";

/// Tests skipped on the device, with why. adb's `shell` SELinux domain may not create socket files
/// (`avc: denied { create } … tclass=sock_file`), and the Android app does not use the desktop's IPC
/// attach: its debug bridge is a content provider (ADR 0009).
const DEVICE_SKIPS: &[&str] = &["ipc::"];

/// The directories of a crate the tests read at run time.
const TEST_DATA_DIRS: &[&str] = &["tests", "testdata"];

const ANDROID_USAGE: &str =
    "cargo xtask android check|apk|apk-check|licences [--check]|all|test [ARGS…]";

/// The licence lists of the About screen, under `crates/app-mobile` (included by `app_mobile::theme`).
const LICENCE_LISTS: [&str; 3] = [
    "licences/rust-crates.txt",
    "licences/android-libraries.txt",
    "licences/pdf-assets.txt",
];

/// Licensee's report for the APK users get (`assembleLiveRelease`).
const LICENSEE_REPORT: &str = "app/build/reports/licensee/androidLiveRelease/artifacts.json";

/// The Gradle tasks of `android check`: the formatter, the licence allowlist, Lint (warnings are
/// errors) and the JVM tests.
const GRADLE_CHECKS: &[&str] = &[
    "spotlessCheck",
    "licensee",
    "lint",
    "testLiveDebugUnitTest",
    "testDemoDebugUnitTest",
];

/// The APKs `android apk` assembles: the release build users get, and the debug build agents drive.
const APK_TASKS: &[&str] = &["assembleLiveRelease", "assembleDemoDebug"];

/// Native libraries an APK may carry besides `libdail.so`, each with why (AGENTS.md §5, ADR 0009).
const PREBUILT_ALLOWLIST: &[(&str, &str)] = &[(
    "libandroidx.graphics.path.so",
    "androidx.graphics:graphics-path (Apache-2.0), Compose's path iterator; built by AOSP",
)];

/// The permissions every APK asks for, exactly. A new one is a decision: add it here with its item.
/// The last is AndroidX core's guard for receivers registered as not exported.
const APK_PERMISSIONS: &[&str] = &[
    "android.permission.INTERNET",
    // A4: the send service (`shortService` from Android 14, `dataSync` before).
    "android.permission.FOREGROUND_SERVICE",
    "android.permission.FOREGROUND_SERVICE_DATA_SYNC",
    // A5: the periodic check is kept across reboots; a network change syncs.
    "android.permission.RECEIVE_BOOT_COMPLETED",
    "android.permission.ACCESS_NETWORK_STATE",
    // A5: notifications (asked for at run time from Android 13).
    "android.permission.POST_NOTIFICATIONS",
    // A5: push, a `specialUse` foreground service.
    "android.permission.FOREGROUND_SERVICE_SPECIAL_USE",
    "{app}.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION",
];

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("verify") => verify(&args.collect::<Vec<_>>()),
        Some("ios") => ios::run(&args.collect::<Vec<_>>()),
        Some("release") => release::run(&args.collect::<Vec<_>>()),
        Some("android") => match args.next().as_deref() {
            Some("check") => android_check(),
            Some("apk") => android_apk(),
            Some("apk-check") => apk_check(),
            Some("all") => android_check()
                .and_then(|()| android_apk())
                .and_then(|()| apk_check()),
            Some("licences") => licences(args.next().as_deref() == Some("--check")),
            Some("test") => android_test(&args.collect::<Vec<_>>()),
            _ => {
                let mut err = std::io::stderr().lock();
                writeln!(err, "usage: {ANDROID_USAGE}")?;
                bail!("unknown or missing android task")
            }
        },
        _ => {
            let mut err = std::io::stderr().lock();
            writeln!(err, "usage: cargo xtask verify [{}]…", gate_names("|"))?;
            writeln!(err, "       {ANDROID_USAGE}")?;
            writeln!(err, "       {}", ios::USAGE)?;
            writeln!(err, "       {}", release::USAGE)?;
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

pub(crate) fn workspace_root() -> Result<PathBuf> {
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
    let missing = missing_targets(&root, ANDROID_TARGETS)?;
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
    gradle(&root, GRADLE_CHECKS)?;
    licences(true)
}

/// Writes [`LICENCE_LISTS`], or with `check` fails when one differs from what it would write.
fn licences(check: bool) -> Result<()> {
    let root = workspace_root()?;
    let report = root.join("android").join(LICENSEE_REPORT);
    if !report.exists() {
        gradle(&root, &["licensee"])?;
    }
    let lists = [
        rust_crates(&root)?,
        android_libraries(&report)?,
        pdf_assets(&root)?,
    ];
    let mut err = std::io::stderr().lock();
    let mut stale = Vec::new();
    for (name, list) in LICENCE_LISTS.iter().zip(lists) {
        let path = root.join("crates/app-mobile").join(name);
        if check {
            if std::fs::read_to_string(&path).ok().as_deref() != Some(list.as_str()) {
                stale.push(*name);
            }
        } else {
            std::fs::write(&path, list).with_context(|| format!("writing {}", path.display()))?;
            writeln!(err, "android licences: wrote crates/app-mobile/{name}")?;
        }
    }
    if !stale.is_empty() {
        bail!(
            "stale licence lists: {} (run `cargo xtask android licences`)",
            stale.join(", ")
        );
    }
    Ok(())
}

/// The data hayro embeds, with its licences (ADR 0012): the Foxit fonts and the colour profiles of
/// hayro-interpret, and the Adobe CMaps of hayro-cmap, read from the checked-out sources.
fn pdf_assets(root: &Path) -> Result<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(&cargo)
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(root)
        .output()
        .context("failed to spawn `cargo metadata`")?;
    if !output.status.success() {
        bail!("`cargo metadata` failed with {}", output.status);
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let dir = |name: &str| -> Result<std::path::PathBuf> {
        let manifest = metadata["packages"]
            .as_array()
            .context("no packages")?
            .iter()
            .find(|package| package["name"] == name)
            .and_then(|package| package["manifest_path"].as_str())
            .with_context(|| format!("no package {name}"))?;
        Ok(Path::new(manifest)
            .parent()
            .context("manifest without a directory")?
            .join("assets"))
    };
    let read = |path: std::path::PathBuf| {
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
    };
    let interpret = dir("hayro-interpret")?;
    let cmap = dir("hayro-cmap")?;
    Ok(format!(
        "PDF fonts and colour profiles (hayro-interpret)\n\n{}\n{}\n{}\nAdobe CMaps (hayro-cmap)\n\n{}",
        read(interpret.join("README.md"))?.trim_end(),
        read(interpret.join("LICENSE_FOXIT"))?,
        read(interpret.join("CGATS_LICENSE.txt"))?,
        read(cmap.join("LICENSE.txt"))?,
    ))
}

/// `name version licence` for every package `libdail.so` links on [`ANDROID_TARGETS`]: the normal
/// dependencies of `app-android` without features, less the workspace's own crates and the proc
/// macros (and what only they use), which run at build time.
fn rust_crates(root: &Path) -> Result<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut args = vec!["metadata", "--format-version", "1", "--locked"];
    for target in ANDROID_TARGETS {
        args.extend(["--filter-platform", target]);
    }
    let output = Command::new(&cargo)
        .args(&args)
        .current_dir(root)
        .output()
        .context("failed to spawn `cargo metadata`")?;
    if !output.status.success() {
        bail!("`cargo metadata` failed with {}", output.status);
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let packages: std::collections::HashMap<&str, &serde_json::Value> = metadata["packages"]
        .as_array()
        .context("no packages")?
        .iter()
        .filter_map(|package| Some((package["id"].as_str()?, package)))
        .collect();
    let members: Vec<&str> = metadata["workspace_members"]
        .as_array()
        .context("no workspace members")?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    let nodes: std::collections::HashMap<&str, &serde_json::Value> = metadata["resolve"]["nodes"]
        .as_array()
        .context("no resolve graph")?
        .iter()
        .filter_map(|node| Some((node["id"].as_str()?, node)))
        .collect();
    let proc_macro = |id: &str| {
        packages.get(id).is_some_and(|package| {
            package["targets"].as_array().is_some_and(|targets| {
                targets.iter().any(|target| {
                    target["kind"]
                        .as_array()
                        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "proc-macro"))
                })
            })
        })
    };
    let start = members
        .iter()
        .copied()
        .find(|id| packages.get(id).is_some_and(|p| p["name"] == "app-android"))
        .context("app-android is not a workspace member")?;
    let mut seen = std::collections::BTreeSet::new();
    let mut queue = vec![start];
    while let Some(id) = queue.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(deps) = nodes.get(id).and_then(|node| node["deps"].as_array()) else {
            continue;
        };
        for dep in deps {
            let normal = dep["dep_kinds"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind["kind"].is_null()));
            // `app-testkit` is a normal dependency only behind the `demo` feature.
            let Some(dep_id) = dep["pkg"].as_str() else {
                continue;
            };
            if normal && !proc_macro(dep_id) && dep["name"] != "app_testkit" {
                queue.push(dep_id);
            }
        }
    }
    let mut lines: Vec<String> = seen
        .into_iter()
        .filter(|id| !members.contains(id))
        .filter_map(|id| packages.get(id))
        .map(|package| {
            let licence = package["license"].as_str().unwrap_or("see its repository");
            format!(
                "{} {} {licence}",
                package["name"].as_str().unwrap_or_default(),
                package["version"].as_str().unwrap_or_default()
            )
        })
        .collect();
    lines.sort();
    lines.dedup();
    Ok(lines.join("\n") + "\n")
}

/// `group:artifact version licences` for every library in licensee's `report`.
fn android_libraries(report: &Path) -> Result<String> {
    let text =
        std::fs::read_to_string(report).with_context(|| format!("reading {}", report.display()))?;
    let artifacts: serde_json::Value = serde_json::from_str(&text)?;
    let mut lines: Vec<String> = artifacts
        .as_array()
        .context("licensee's report is not a list")?
        .iter()
        .map(|artifact| {
            let licences: Vec<&str> = artifact["spdxLicenses"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|licence| licence["identifier"].as_str())
                        .collect()
                })
                .unwrap_or_default();
            format!(
                "{}:{} {} {}",
                artifact["groupId"].as_str().unwrap_or_default(),
                artifact["artifactId"].as_str().unwrap_or_default(),
                artifact["version"].as_str().unwrap_or_default(),
                licences.join(" OR ")
            )
        })
        .collect();
    lines.sort();
    Ok(lines.join("\n") + "\n")
}

/// Assembles [`APK_TASKS`].
fn android_apk() -> Result<()> {
    gradle(&workspace_root()?, APK_TASKS)
}

/// Runs `android/gradlew` with `tasks`. Without a JDK or the SDK it reports "cannot run".
fn gradle(root: &Path, tasks: &[&str]) -> Result<()> {
    let project = root.join("android");
    let mut err = std::io::stderr().lock();
    if let Some(reason) = gradle_blocker(&project) {
        writeln!(err, "android: cannot run: {reason}")?;
        bail!("android gradle tasks cannot run");
    }
    let gradlew = project.join(if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    });
    writeln!(err, "android: gradlew {}", tasks.join(" "))?;
    let status = Command::new(&gradlew)
        .args(tasks)
        .current_dir(&project)
        .status()
        .with_context(|| format!("failed to spawn {}", gradlew.display()))?;
    if !status.success() {
        bail!("`gradlew {}` failed with {status}", tasks.join(" "));
    }
    Ok(())
}

/// Why Gradle cannot run, if it cannot: no JDK, or no Android SDK.
fn gradle_blocker(project: &Path) -> Option<String> {
    let java = std::env::var_os("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("bin").join("java"))
        .unwrap_or_else(|| PathBuf::from("java"));
    if !Command::new(&java)
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        return Some("no JDK found (install JDK 17 and set JAVA_HOME)".to_owned());
    }
    if sdk_dir(project).is_none() {
        return Some(
            "no Android SDK found (set ANDROID_HOME, or sdk.dir in android/local.properties)"
                .to_owned(),
        );
    }
    None
}

/// The Android SDK: `ANDROID_HOME`, `ANDROID_SDK_ROOT`, else `sdk.dir` in `local.properties`.
fn sdk_dir(project: &Path) -> Option<PathBuf> {
    ["ANDROID_HOME", "ANDROID_SDK_ROOT"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .chain(
            std::fs::read_to_string(project.join("local.properties"))
                .ok()
                .and_then(|text| {
                    text.lines()
                        .find_map(|line| line.strip_prefix("sdk.dir=").map(PathBuf::from))
                }),
        )
        .find(|dir| dir.is_dir())
}

/// The newest build-tools directory of the SDK (zipalign, aapt2).
fn build_tools(sdk: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(sdk.join("build-tools"))
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|dir| dir.join("aapt2").is_file() || dir.join("aapt2.exe").is_file())
        .collect();
    dirs.sort_by_key(|dir| {
        dir.file_name()
            .and_then(|name| name.to_str())
            .map(|name| {
                name.split(['.', '-'])
                    .map(|part| part.parse::<u32>().unwrap_or(0))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    dirs.pop()
}

/// Checks every APK under `android/app/build/outputs/apk` (§4.6 of the Android plan).
fn apk_check() -> Result<()> {
    let root = workspace_root()?;
    let project = root.join("android");
    let mut err = std::io::stderr().lock();
    let Some(tools) = sdk_dir(&project).and_then(|sdk| build_tools(&sdk)) else {
        writeln!(
            err,
            "android apk-check: cannot run: no SDK build-tools found (sdkmanager \"build-tools;36.1.0\")"
        )?;
        bail!("android apk-check cannot run");
    };
    let apks = find_apks(&project.join("app/build/outputs/apk"))?;
    if apks.is_empty() {
        writeln!(
            err,
            "android apk-check: cannot run: no APKs (run `cargo xtask android apk`)"
        )?;
        bail!("android apk-check cannot run");
    }
    let mut problems = Vec::new();
    for apk in &apks {
        let name = apk
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        for problem in check_one_apk(&tools, apk)? {
            problems.push(format!("{name}: {problem}"));
        }
        writeln!(err, "android apk-check: {name}")?;
    }
    if !problems.is_empty() {
        for problem in &problems {
            writeln!(err, "  {problem}")?;
        }
        bail!("android apk-check found {} problems", problems.len());
    }
    writeln!(err, "android apk-check: {} APKs passed", apks.len())?;
    Ok(())
}

fn find_apks(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut apks = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(apks);
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            apks.extend(find_apks(&path)?);
        } else if path.extension().is_some_and(|ext| ext == "apk")
            // Instrumented-test APKs carry no app and never ship.
            && !path.to_string_lossy().contains("androidTest")
        {
            apks.push(path);
        }
    }
    apks.sort();
    Ok(apks)
}

/// Runs an SDK tool and returns its stdout.
fn tool_output(tool: &Path, args: &[&str], apk: &Path) -> Result<(bool, String)> {
    let output = Command::new(tool)
        .args(args)
        .arg(apk)
        .output()
        .with_context(|| format!("failed to spawn {}", tool.display()))?;
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    ))
}

/// What is wrong with one APK.
fn check_one_apk(tools: &Path, apk: &Path) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    let release = apk.to_string_lossy().contains("release");

    // `zipalign -c -P 16 -v 4` lists every entry and checks each `.so` against 16 KB pages.
    let (aligned, listing) =
        tool_output(&tools.join("zipalign"), &["-c", "-P", "16", "-v", "4"], apk)?;
    if !aligned {
        problems.push("not aligned (zipalign -c -P 16 4)".to_owned());
    }
    for line in listing.lines() {
        let Some(entry) = line.split_whitespace().nth(1) else {
            continue;
        };
        let Some(lib) = entry.strip_prefix("lib/") else {
            continue;
        };
        let file = lib.rsplit('/').next().unwrap_or(lib);
        let allowed =
            file == "libdail.so" || PREBUILT_ALLOWLIST.iter().any(|(name, _)| *name == file);
        if !allowed {
            problems.push(format!("native code not allowed: {entry}"));
        }
    }
    if !listing.contains("libdail.so") {
        problems.push("no libdail.so".to_owned());
    }

    let aapt2 = tools.join("aapt2");
    let (_, badging) = tool_output(&aapt2, &["dump", "badging"], apk)?;
    let app = badging
        .lines()
        .find_map(|line| line.strip_prefix("package: name='"))
        .and_then(|rest| rest.split('\'').next())
        .unwrap_or_default()
        .to_owned();
    let (_, permissions) = tool_output(&aapt2, &["dump", "permissions"], apk)?;
    let mut asked: Vec<String> = permissions
        .lines()
        .filter_map(|line| line.strip_prefix("uses-permission: name='"))
        .filter_map(|rest| rest.split('\'').next())
        .map(str::to_owned)
        .collect();
    asked.sort();
    let mut pinned: Vec<String> = APK_PERMISSIONS
        .iter()
        .map(|permission| permission.replace("{app}", &app))
        .collect();
    pinned.sort();
    if asked != pinned {
        problems.push(format!("permissions {asked:?}, pinned {pinned:?}"));
    }

    if release {
        if badging
            .lines()
            .any(|line| line.trim() == "application-debuggable")
        {
            problems.push("a release build is debuggable".to_owned());
        }
        let (_, manifest) = tool_output(
            &aapt2,
            &["dump", "xmltree", "--file", "AndroidManifest.xml"],
            apk,
        )?;
        if manifest.contains("DebugBridge") {
            problems.push("a release build carries the debug bridge".to_owned());
        }
    }
    Ok(problems)
}

/// The Android targets whose standard library is not installed for the active toolchain.
pub(crate) fn missing_targets(root: &Path, targets: &[&'static str]) -> Result<Vec<&'static str>> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let mut missing = Vec::new();
    for target in targets {
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

/// Builds the test binaries of [`ANDROID_TEST_CRATES`] for [`ANDROID_TEST_TARGET`], pushes them to the
/// device adb sees and runs each there, passing `test_args` on. A missing tool or device is reported
/// as "cannot run".
fn android_test(test_args: &[String]) -> Result<()> {
    let root = workspace_root()?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut err = std::io::stderr().lock();
    let adb = adb_path();
    if let Some(reason) = android_test_blocker(&cargo, &adb, &root)? {
        writeln!(err, "android test: cannot run: {reason}")?;
        bail!("android test cannot run");
    }

    // cargo-ndk's environment (linker, CC, AR for the NDK) on a plain `cargo test`: `cargo ndk`
    // swallows cargo's stdout, and the JSON there names the binaries.
    let output = Command::new(&cargo)
        .args(["ndk-env", "--json", "-t", ANDROID_TEST_TARGET])
        .args(["--platform", ANDROID_TEST_API])
        .current_dir(&root)
        .output()
        .context("failed to spawn `cargo ndk-env`")?;
    if !output.status.success() {
        err.write_all(&output.stderr)?;
        bail!("`cargo ndk-env` failed with {}", output.status);
    }
    let env: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&output.stdout)
        .context("`cargo ndk-env --json` printed no object")?;

    let mut args = vec![
        "test",
        "--no-run",
        "--target",
        ANDROID_TEST_TARGET,
        "--message-format=json-render-diagnostics",
    ];
    for name in ANDROID_TEST_CRATES {
        args.extend(["-p", name]);
    }
    writeln!(err, "android test: cargo {}", args.join(" "))?;
    let mut build = Command::new(&cargo);
    build
        .args(&args)
        .current_dir(&root)
        // Line tables keep backtraces readable at a fraction of full debug info's size to push.
        .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
        .stderr(std::process::Stdio::inherit());
    for (key, value) in &env {
        if let Some(value) = value.as_str() {
            build.env(key, value);
        }
    }
    // cargo-ndk's linker shim works only under `cargo ndk`; link with the NDK's clang wrapper for the
    // API level instead.
    let clang = env
        .get("CLANG_PATH")
        .and_then(serde_json::Value::as_str)
        .context("`cargo ndk-env` named no CLANG_PATH")?;
    let linker =
        Path::new(clang).with_file_name(format!("{ANDROID_TEST_TARGET}{ANDROID_TEST_API}-clang"));
    build
        .env("CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER", &linker)
        .env_remove("CARGO_TARGET_X86_64_LINUX_ANDROID_RUNNER");
    let output = build
        .output()
        .with_context(|| format!("failed to spawn `cargo {}`", args.join(" ")))?;
    if !output.status.success() {
        bail!("building the Android tests failed with {}", output.status);
    }
    let binaries = test_binaries(&output.stdout, &root)?;

    adb_run(
        &adb,
        &[
            "shell",
            &format!("rm -rf {DEVICE_ROOT} && mkdir -p {DEVICE_ROOT}/bin {DEVICE_ROOT}/tmp"),
        ],
    )?;
    for name in ANDROID_TEST_CRATES {
        let local = root.join("crates").join(name);
        for dir in TEST_DATA_DIRS {
            if local.join(dir).is_dir() {
                let remote = format!("{DEVICE_ROOT}/crates/{name}");
                adb_run(&adb, &["shell", &format!("mkdir -p {remote}")])?;
                adb_run(&adb, &["push", "-q", &path_str(&local.join(dir))?, &remote])?;
            }
        }
    }

    let mut failed = Vec::new();
    for binary in &binaries {
        let file = binary
            .path
            .file_name()
            .context("test binary without a file name")?
            .to_string_lossy()
            .into_owned();
        let remote = format!("{DEVICE_ROOT}/bin/{file}");
        writeln!(err, "android test: {} ({file})", binary.crate_name)?;
        adb_run(&adb, &["push", "-q", &path_str(&binary.path)?, &remote])?;
        let crate_dir = format!("{DEVICE_ROOT}/crates/{}", binary.crate_name);
        let mut line = format!(
            "chmod 755 {remote} && mkdir -p {crate_dir} && cd {crate_dir} && \
             CARGO_MANIFEST_DIR={crate_dir} INSTA_WORKSPACE_ROOT={DEVICE_ROOT} INSTA_UPDATE=no \
             TMPDIR={DEVICE_ROOT}/tmp RUST_BACKTRACE=1 {remote}"
        );
        for skip in DEVICE_SKIPS {
            line.push_str(&format!(" --skip {}", shell_quote(skip)));
        }
        for arg in test_args {
            line.push(' ');
            line.push_str(&shell_quote(arg));
        }
        // adb (API 24+) returns the remote command's exit status.
        let status = Command::new(&adb)
            .args(["shell", &line])
            .status()
            .context("failed to spawn adb")?;
        if !status.success() {
            failed.push(format!("{} ({file})", binary.crate_name));
        }
    }
    if !failed.is_empty() {
        bail!("android tests failed: {}", failed.join(", "));
    }
    writeln!(err, "android test: {} test binaries passed", binaries.len())?;
    Ok(())
}

/// Why `cargo xtask android test` cannot run, if it cannot: cargo-ndk, the rustup target, the NDK or
/// a device is missing.
fn android_test_blocker(cargo: &str, adb: &Path, root: &Path) -> Result<Option<String>> {
    let ndk = Command::new(cargo)
        .args(["ndk", "--version"])
        .current_dir(root)
        .output();
    if !ndk.is_ok_and(|output| output.status.success()) {
        return Ok(Some(
            "cargo-ndk not installed (cargo install cargo-ndk)".to_owned(),
        ));
    }
    if missing_targets(root, ANDROID_TARGETS)?.contains(&ANDROID_TEST_TARGET) {
        return Ok(Some(format!(
            "Rust target not installed: {ANDROID_TEST_TARGET} (rustup target add {ANDROID_TEST_TARGET})"
        )));
    }
    let env = Command::new(cargo)
        .args(["ndk-env", "-t", ANDROID_TEST_TARGET])
        .current_dir(root)
        .output();
    if !env.is_ok_and(|output| output.status.success()) {
        return Ok(Some(
            "no Android NDK found (set ANDROID_NDK_HOME or install one under ANDROID_HOME/ndk)"
                .to_owned(),
        ));
    }
    let state = Command::new(adb).arg("get-state").output();
    match state {
        Err(_) => Ok(Some(format!(
            "adb not found ({}; install the SDK's platform-tools)",
            adb.display()
        ))),
        Ok(output) if String::from_utf8_lossy(&output.stdout).trim() == "device" => Ok(None),
        Ok(_) => Ok(Some(
            "no device: start an x86_64 emulator (or set ANDROID_SERIAL to pick one)".to_owned(),
        )),
    }
}

/// adb from the SDK (`ANDROID_HOME`, then `ANDROID_SDK_ROOT`), else from `PATH`.
fn adb_path() -> PathBuf {
    ["ANDROID_HOME", "ANDROID_SDK_ROOT"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|sdk| PathBuf::from(sdk).join("platform-tools").join("adb"))
        .find(|adb| adb.is_file())
        .unwrap_or_else(|| PathBuf::from("adb"))
}

fn adb_run(adb: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new(adb)
        .args(args)
        .status()
        .context("failed to spawn adb")?;
    if !status.success() {
        bail!("`adb {}` failed with {status}", args.join(" "));
    }
    Ok(())
}

/// One test binary from cargo's JSON messages.
struct TestBinary {
    crate_name: String,
    path: PathBuf,
}

/// The test executables in cargo's `--message-format=json` output, with the workspace crate (its
/// directory name under `crates/`) each belongs to.
fn test_binaries(stdout: &[u8], root: &Path) -> Result<Vec<TestBinary>> {
    let crates_dir = root.join("crates");
    let mut binaries = Vec::new();
    for line in stdout.split(|&byte| byte == b'\n') {
        let Ok(message) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        let (Some(executable), Some(manifest)) = (
            message["executable"].as_str(),
            message["manifest_path"].as_str(),
        ) else {
            continue;
        };
        if message["reason"] != "compiler-artifact" || message["profile"]["test"] != true {
            continue;
        }
        let crate_name = Path::new(manifest)
            .parent()
            .and_then(|dir| dir.strip_prefix(&crates_dir).ok())
            .map(|dir| dir.to_string_lossy().into_owned())
            .with_context(|| format!("{manifest} is not a crate under crates/"))?;
        binaries.push(TestBinary {
            crate_name,
            path: PathBuf::from(executable),
        });
    }
    if binaries.is_empty() {
        bail!("cargo built no test binaries");
    }
    Ok(binaries)
}

fn path_str(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .with_context(|| format!("{} is not UTF-8", path.display()))
}

/// Quotes `arg` for the device's shell.
fn shell_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', "'\\''"))
}
