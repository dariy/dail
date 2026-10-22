# 0009 — Android: a Compose app over a Rust presenter

- Status: accepted
- Date: 2026-10-22
- Plan: `docs/plans/2026-10-22-android.md` (§1–§4; ROADMAP Wave 4)

## Context

dail runs on Ubuntu and Windows (egui, ratatui). A daily-drivable phone app should run the same engine, registry,
keymap, palette flows and agent policy, with no rules re-derived in another language. A mail client is mostly typing,
so the platform's text input (any IME: CJK, swipe typing, voice) is not optional. AGENTS.md §5 forbids compiling or
linking C/C++, which rules out GameActivity (android-activity compiles its C++ with `cc`) and JNA (UniFFI's Kotlin
loader ships `libjnidispatch.so`). The options weighed are in plan §1, the platform facts in plan §2.

## Decision

- **UI.** Kotlin + Jetpack Compose draw screen models ("frames") that Rust computes. Not egui on NativeActivity: winit
  0.30 has no IME composition there. Not a WebView (Tauri, Dioxus): hostile HTML would meet a browser engine. Not
  UniFFI: JNA.
- **Two crates.**
  - `app-mobile` ← app-core, app-front, app-theme. Toolkit-free, no tokio, no JNI, like `app-tui` with JSON frames
    instead of a terminal buffer: `Mobile` with `frame(&state) -> Frame`, `event(UiEvent, &state) -> Vec<Request>`,
    `replies` and `externals`; every model `Serialize + JsonSchema`. It could serve an iOS host later.
  - `app-android` ← app-core, app-io, app-sync, app-mobile (and app-testkit behind its `demo` feature, as ADR 0001
    allows for app-cli). A cdylib + rlib, `[lib] name = "dail"` (`libdail.so`): the `Host` (session, presenter) and
    six JNI calls (`open`, `event`, `frame`, `blob`, `background`, `close`) through `jni` 0.22, carrying the JSON the
    command bus speaks. It builds on every host; the Android-only parts (Keystore store, ndk-context, logcat) sit
    behind `cfg(target_os = "android")`.
  - app-gui, app-tui and app-cli depend on neither.
- **The lint exception.** `app-android` is the only crate whose `[lints]` is not `workspace = true`: the workspace
  table with `unsafe_code = "deny"` instead of `forbid`. Only `src/jni.rs` allows unsafe code (`#![allow(unsafe_code)]`
  with a `// reason:` line): JNI exports need `#[unsafe(export_name)]`, and `JavaVM` and ndk-context take raw
  pointers. A test compares the table with the workspace's.
- **Shared rules move first** (A0): the sanitizer to `app_front::text`, accessible names to `app_front::a11y`, the
  pane choice to `app_front::panes`, image decoding and its limits to `app_io::images`.
- **Origin.** The app is the user's graphical front-end, in-process like the GUI: `Origin::Gui`. Background checks
  submit as `runtime`.
- **Process.** One session per process, opened lazily and kept while the process lives. A foreground service carries a
  send through its undo window; a one-off job flushes queued work when the app leaves the screen; a periodic
  JobScheduler job checks mail (`background_sync` 1, default, every `background_interval_mins` = 30); push
  (`background_sync` 2) is a `specialUse` foreground service around the IMAP IDLE watches.
- **Platform.** minSdk 29, compile and target SDK 36; arm64-v8a and x86_64; NDK r28+ with 16 KB pages; no Google Play
  Services, Firebase or WorkManager. Application ID `net.darii.dail`. The first release is a signed APK per ABI on
  GitHub releases.
- **Secrets and files.** keyring 4 with `android-native-keyring-store` (Keystore-wrapped). The profile lives in
  `filesDir/profiles/default` with backups off. Picked and shared files are copied into the app before `attach_file`;
  saving uses the Storage Access Framework.
- **Kotlin.** Kotlin holds no rules: it draws frames, returns events and carries out host actions. The APK carries no
  native code but `libdail.so`. Gradle dependencies are pinned in a version catalog with checksum verification and a
  licence allowlist (the Gradle side of `cargo deny`).
- **Exception to channel parity (AGENTS.md §1).** Release builds on Android have no agent channel: no MCP server, no
  approvals screen. No on-device client could use one (plan §4.7: on-device models get 4k–10k tokens of context, the
  MCP tool list is ~22k; AppFunctions are callable only by privileged apps). The palette reaches every registry
  command (a parity walk tests it). A debug-only adb bridge, answering only the shell UID, submits commands as `cli`
  or `mcp`, so agents drive the app in tests.
- **Verification.** A gate `android` after `smoke`, run by `cargo xtask verify android` and not by default: it needs
  the NDK (and from A2 the SDK). A missing tool is reported as "cannot run". CI gets an `android` job and an emulator
  job.

## Consequences

- Every Android rule is Rust, tested headless like the TUI's; Kotlin is a renderer and an adapter to the platform.
- Kotlin and Gradle enter the repository, and CI gains two Android jobs.
- One crate deviates from the workspace lints, confined to one file and pinned by a test.
- Agents keep working on the desktop; their triage reaches the phone through the servers. A phone-side channel
  (cross-device approvals, AppFunctions, a compact MCP profile) needs its own ADR.
