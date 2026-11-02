# dail — store-readiness review

Reviewed on 2026-11-02 at commit `b3e50f2`, with every code anchor checked again at `c4dd075` (branch `go/misc`, 38
commits ahead of `main`). The question: what stands between dail and a production release on Google Play and F-Droid?
The plan that follows from this review is `ROADMAP.md`, Wave 11.

**How it was checked.** The code and docs were read, CI and PR results came from `gh`, and the published v0.1.3 APK was
inspected. No gate was run for this review and no device was used. Findings marked *not reproduced* come from reading
the code. Section 6 lists what this review did not establish.

## 1. Verdict

The engine is strong and most of Play's technical baseline is met. No architectural change is needed. What stands
between dail and a store is a set of specific gaps: real servers that cannot connect, crash and data-loss paths in the
Android and session layers, the store packaging itself, and no recorded real-device check.

## 2. What is already solid

- A sans-IO engine, about 1,180 Rust tests, a palette parity walk, and a contract between Rust frames and Kotlin.
- targetSdk 36, Play's current minimum; minSdk 29; 16 KB pages, which `apk-check` enforces; arm64 and x86_64; an APK of
  about 23 MB.
- No Play Services, Firebase, trackers or WebView. Dependencies are FLOSS, with a licence allowlist on both the Cargo
  and Gradle sides.
- No plaintext mail mode (`app-core/src/account.rs:148`). Passwords are Keystore-wrapped, backups are off, `file://`
  shares are refused, and permissions are pinned by `xtask`.
- The release certificate on the v0.1.3 APK runs until March 2054.

## 3. Findings

Severity: **[H]** blocks a store release or harms users, **[M]** should be fixed before production, **[L]** polish. Each
finding names its Wave 11 item.

### High

- **H1. Many servers cannot connect, and the crypto is unreviewed.** `crates/app-tls/src/lib.rs:37` offers ChaCha20-Poly1305
  and X25519 only. ADR 0003 records that T-Online, Namecheap Private Email and Microsoft 365 therefore fail the
  handshake, and any other server without those algorithms will too. ADR 0003 also says about 1,900 lines of unreviewed
  crypto need an outside review before a public release; none has happened. *Fix:* RustCrypto primitives, ADR 0014
  (STORE-11).
- **H2. Foreground-service starts can crash the app** *(not reproduced)*. `SendService.start`
  (`SendService.kt:78`, called at `MainActivity.kt:209`) is unguarded, while `PushService.start` has `runCatching`.
  `startForeground` at `SendService.kt:35` and `PushService.kt:36` is unguarded too, and `PushService` is `START_STICKY`,
  so it also runs on a restart from the background. Android 12+ can refuse these calls with an exception. *Fix:* guard
  them and fall back to the flush job (STORE-3).
- **H3. A panic stops an account silently.** An effect runs in `worker()` (`session.rs:543`) with no panic handling. If
  it panics, the lane's worker dies, and later effects are dropped with one log line (`session.rs:387`), so that account
  stops syncing until the process restarts. `set_hook` appears only in `app-tui`, so panics on Android runtime threads
  leave no trace. *Fix:* STORE-4, with a failure screen users can act on (STORE-5).
- **H4. An upgrade can lose drafts and queued work without saying so.** `MIGRATIONS` is empty
  (`migrations.rs:17`), rows are JSON, and an unreadable row is skipped with a warning (`cache.rs:167`), drafts and
  outbox included. Serialization stability covers commands and events only. Once store users exist, every update
  upgrades their data. *Fix:* a golden profile, and quarantine instead of a drop (STORE-6).
- **H5. Hostile mail is not fuzzed** *(not reproduced)*. Nothing fuzzes the MIME, HTML, image or PDF parsers. Panics
  in `store_blob` are caught as join errors, but a stack overflow aborts the process, and the hayro overflow (ADR 0012)
  shows this class is real. Bodies are parsed at sync, so one such message could crash the app on every sync. *Fix:*
  fuzzing and a crash-loop breaker (STORE-7).
- **H6. The Android composer is not fit for a phone.** MISC-31: the fields do not scroll, the caret goes under the
  keyboard, and auto-saves disturb the editor. A mail client cannot ship like this.
- **H7. Nothing store-facing exists.** There is no AAB. `versionCode` (`build.gradle.kts:48`) collides at patch 100 and
  is the same for every ABI, while F-Droid wants one per ABI. The launcher icon is a placeholder
  (`ic_launcher.xml:2`), notifications use framework icons (`Notifier.kt:95`), and there is no store metadata, no phone
  screenshots and no privacy policy. *Fix:* stage D.
- **H8. No device check is recorded.** A7 (`ROADMAP.md:521`) is open, and the claim that push notifies within about a
  minute is unmeasured. dail holds no wake lock, sets no TCP keepalive and refreshes IDLE every 25 minutes
  (`imap.rs:1162`). *Fix:* STORE-23.

### Medium

- **M1. CI is behind.** `go/misc` has 38 commits that never ran the Windows, iOS and emulator jobs. The last CI run on
  `main` was on 2026-10-29 (STORE-1).
- **M2. Bot PRs fail.** PR #21 fails on a stale `licences/rust-crates.txt`. PR #19 failed Gradle's dependency
  verification, and CI on the refresh commit stopped at `action_required` (STORE-2).
- **M3. An update banner in a Play build.** `crates/app-android/src/lib.rs:15` reports an install kind, so checks run and
  a banner links to GitHub. Play bans updating outside its own mechanism. A link is not explicitly banned, but it is a
  needless risk, and the check adds a network destination to explain. `install: None` turns both off (STORE-15).
- **M4. Mobile data use.** `prefetch_bodies` (`sync.rs:848`) fetches `BODY.PEEK[]` (`imap.rs:727`), the whole message
  with its attachments, for the 20 newest unread, with no size limit. Nothing in the repository checks for a metered
  network (STORE-8).
- **M5. Logs grow forever.** `android.log` (`logging.rs:56`) has no rotation, and `audit.rs:34` writes one file per day
  with no cleanup (STORE-10).
- **M6. Encrypted dependency block.** The v0.1.3 APK carries block `0x504b4453`, which F-Droid is known to flag
  (STORE-16; see section 6).
- **M7. An avoidable Play declaration.** `dataSync` is used only below Android 14, where a typed start is not needed
  (STORE-14).
- **M8. Native crashes will have no symbols.** `strip = true` (`Cargo.toml:229`) leaves Play's Android vitals, the only
  crash reporting dail would have, with raw addresses (STORE-16).
- **M9. Large attachments go through memory.** *Save* copies a whole part through a JNI byte array (`Host.blob`,
  `MainActivity.write`), and the out-of-memory case is not handled (STORE-9).
- **M10. Setup does not guide users.** Gmail gets an *App Password* label (`setup.rs:502`) with no link, and Microsoft
  addresses fail late instead of being told up front (STORE-12).
- **M11. The UI has no regression test.** There are 16 Kotlin tests: contract, viewer geometry and the debug bridge. None
  draws Compose UI, despite Wave 4's Robolectric decision (STORE-9b).
- **M12. The failure screen is a dead end.** `Screen.Failed` shows raw text with no way forward (STORE-5).
- **M13. Plain-text mail wraps raggedly on a phone** (MISC-28).

### Low

- **L1.** `setup.rs:509` is hard-coded English, though MISC-32 is ticked (STORE-1).

## 4. Docs that disagree with the code

Each is corrected in STORE-1, or by the item that closes the gap.

- `ROADMAP.md`, Wave 7 exit criteria: "Cargo and Gradle update PRs pass CI unattended" does not hold (M2).
- `ROADMAP.md:390`: "Compose tests on the JVM (Robolectric)" never landed (M11).
- ADR 0003: an outside review "before a public release", yet releases are public (H1).
- AGENTS.md §5: "The APK carries no native code but `libdail.so`", while `xtask` allowlists
  `libandroidx.graphics.path.so`, which is in the v0.1.3 APK.
- README *Releases* describes run artifacts only, though Wave 7 publishes GitHub Releases. README *Platforms* still calls
  iOS "in progress".
- `ic_launcher.xml:2` says the release icon comes with A7. A7's release item is ticked and the icon is still a placeholder.

## 5. Store rules, as checked on 2026-11-02

| Rule | dail | Item |
|---|---|---|
| Play: new apps and updates target API 36 since 2026-08-31 | Met | — |
| Play: personal accounts created after 2023-11-13 need 12 testers opted in for 14 straight days; review usually takes up to 7 days | To do | STORE-24, 25 |
| Play: foreground-service types are declared and reviewed; `specialUse` only in limited cases | To do | STORE-14, 22 |
| Play: Data safety form and a privacy policy, even if no data is collected | To do | STORE-19 |
| Play: no updating outside Play's own mechanism | To do | STORE-15 |
| Play: reviewers need working credentials for an app behind a login | To do | STORE-22 |
| F-Droid: FLOSS code and toolchain, no proprietary SDKs | Met | — |
| F-Droid: 3 months of active development, unless the app has "proven to have some users" | 2027-01-16 | STORE-26 |
| F-Droid: each ABI is its own build with its own version code, the ABI digit lowest, x86_64 above arm64 | To do | STORE-13 |
| F-Droid: fastlane-style metadata encouraged; short description under 80 characters, changelog at most 500 | To do | STORE-18 |
| F-Droid: no extra executables downloaded without opt-in consent | Met; STORE-15 removes the check | STORE-15 |
| Developer verification: enforced since 2026-09-30 in Brazil, Indonesia, Singapore and Thailand for participating stores; global from 2027; several keys per package allowed | To do | STORE-21, 27 |

Gmail App Passwords are still documented. They need 2-Step Verification and may be missing on work, school and Advanced
Protection accounts, so the store listing should say so.

## 6. What this review did not establish

- **Gates and devices.** No gate was run, so the state of the 38 unmerged commits is unknown. Nothing was tried on a device.
- **H2 and H5.** These come from reading code and from known Android behaviour; neither was reproduced.
- **`specialUse` for IDLE.** A third-party listing shows K-9 Mail holding this permission. That is a precedent, not a
  promise. If Play refuses, push leaves the Play build.
- **Data safety.** "No data collected" is the expected answer, by analogy with Google's example of uploads to a user's
  own Drive or Dropbox account. Confirm it when filling in the form.
- **F-Droid and block `0x504b4453`.** This rests on prior knowledge; the F-Droid pages read here do not mention it. A
  local `fdroid` scan settles it (STORE-26).
- **F-Droid's build server.** Whether it handles compileSdk 37.2, AGP 9.4 and Rust 1.99 is untested.
- **F-Droid after enforcement.** No statement from F-Droid after 2026-09-30 was found, so how its users fared is unknown.
- **Microsoft sign-in.** That Microsoft accounts need OAuth2 is prior knowledge and the roadmap's assumption; it was not
  tried on a live account.
- **RustCrypto's audit status** is prior knowledge, to be checked crate by crate in ADR 0014. What was checked:
  RUSTSEC-2026-0387 affects `crypto-bigint` below 0.7.6, and `Cargo.lock` has 0.7.10.
- **targetSdk 37 in August 2027** is the yearly pattern, not an announcement.
- **CASA cost.** Reported figures vary widely and none is official.

## Sources

Third-party sources are marked.

- [Play target API requirements](https://developer.android.com/google/play/requirements/target-sdk)
- [Play: testing requirements for personal accounts](https://support.google.com/googleplay/android-developer/answer/14151465)
- [Play: foreground service requirements](https://support.google.com/googleplay/android-developer/answer/13392821)
- [Play: Data safety](https://support.google.com/googleplay/android-developer/answer/10787469)
- [Play: Device and Network Abuse](https://support.google.com/googleplay/android-developer/answer/9888379)
- [Play App access guide (third party)](https://buddyboss.com/docs/google-play-issue-need-login-credentials/)
- [K-9 Mail permissions listing (third party)](https://ahao.janney.it.com/en/packages/com.fsck.k9)
- [Android developer verification](https://developer.android.com/developer-verification)
- [Developer verification FAQ](https://developer.android.com/developer-verification/guides/faq)
- [Open-source app registration](https://developer.android.com/developer-verification/guides/open-source-app-registration)
- [F-Droid Inclusion Policy](https://f-droid.org/docs/Inclusion_Policy/)
- [F-Droid Inclusion How-To](https://f-droid.org/docs/Inclusion_How-To/)
- [F-Droid Reproducible Builds](https://f-droid.org/docs/Reproducible_Builds/)
- [Google: app passwords](https://support.google.com/accounts/answer/185833)
- CASA costs (third party, self-reported): [Truto](https://truto.one/google.md), [Indie Hackers](https://www.indiehackers.com/product/mailnotes)
