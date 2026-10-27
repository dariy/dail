# 0011 — Updates: signed releases, checked and installed by dail

- Status: accepted
- Date: 2026-10-27
- Plan: `ROADMAP.md`, Wave 7

## Context

`release.yml` runs by hand and only uploads run artifacts, so nothing is published and an installed dail never
learns that a newer one exists. Users update by hand, or not at all. Checking for updates is a new kind of network
traffic: until now dail only talked to the user's own mail servers. Installing an update runs code that came from the
network. So the source, the integrity check, the privacy rules and who may start an install all need a decision.

## Decision

**Publishing.** A pushed `vX.Y.Z` tag runs `release.yml`. A guard job checks the tag against every version site
(`Cargo.toml`, the Android `versionName`, `ios/project.yml`). The build jobs stay as they are, plus AppImage and MSI.
A `publish` job, in the `release` environment and the only job with `contents: write`, runs
`cargo xtask release manifest` and `gh release create`. Only stable releases from tags; no pre-release channel.

**Manifest.** `manifest.json` and `manifest.json.minisig` are release assets, read from
`https://github.com/dariy/dail/releases/latest/download/`:

```json
{ "version": "0.2.0", "published_ms": 0, "notes_url": "https://…",
  "artifacts": [{ "target": "x86_64-unknown-linux-gnu", "kind": "appimage", "url": "https://…",
                  "sha256": "…", "size": 0 }] }
```

`kind` is one of `deb`, `appimage`, `msi`, `exe`, `apk-<abi>`, `ipa`.

**Signature.** Ed25519 in minisign format. The public key is a constant in the binary. The secret key and its
password are secrets of the `release` environment only, and are never written to logs or to disk outside the job's
temp dir. dail verifies with `minisign-verify` (pure Rust, no dependencies). xtask signs with the `minisign` crate.
The manifest is accepted only if the signature verifies, its trusted comment holds the same version, and that version
is newer than the running one (no downgrades). An artifact is accepted only if its size and sha256 (`sha2`, already in
the tree) match the manifest.

**Transport.** An HTTPS GET in `app-sync`: `hyper`'s HTTP/1.1 client over `tokio-rustls` with `app-tls` and
`webpki-roots`, all already in the tree. GitHub serves TLS 1.3 with X25519 and ChaCha20-Poly1305, which is app-tls
tier 1. Redirects are followed at most 5 times, and only to `github.com`, `objects.githubusercontent.com` and
`release-assets.githubusercontent.com`. The signature is the security boundary; TLS and the host list only reduce
noise.

**Privacy.** Checks are off until the user turns them on (a first-run question, or settings). A check is a plain GET
with `User-Agent: dail/<version>`: no IDs, no counters, no platform beyond what the request reveals. Interval from
settings (default one day). The runtime only submits the check; core decides if it is due, from `Ctx::now_ms`.

**Install.** The runtime finds out how dail was installed (`$APPIMAGE`, the `.deb` package files, the MSI install
dir, otherwise a plain binary). A table maps each install kind to an artifact kind and a strategy:

| Install kind | Strategy |
|---|---|
| AppImage, plain binary / `dail.exe` | Replace the running file (`self-replace`, pure Rust), then ask to restart |
| `.deb` | `pkexec apt-get install ./<file>.deb`; without `pkexec`, show the command |
| MSI | Start `msiexec /i <file>` after the user confirms, then exit |
| Android, iOS | Notify only: open the release page (Obtainium can follow it) |

Downloads go to `<profile>/updates/`. Only the newest download is kept.

**Commands.** `check_for_updates` (agents `any`, no history), `download_update` (agents `any`),
`install_update { confirm }` (Destructive, `AgentAccess::UserOnly`: it runs downloaded code), `skip_update_version`
and `set_update_policy` (agents `any`). The state is `UpdateState` in `AppState`. A failure becomes
`UpdateStatus::Failed { reason }` in state and is never just logged.

**Dependencies of the repository.** Dependabot opens grouped PRs for `cargo` and `gradle` as well as actions. A
workflow regenerates Gradle's verification metadata on those PRs. A monthly workflow opens a toolchain PR (Rust, NDK,
Xcode). A nightly `cargo deny check advisories` on `main` opens an issue when it fails. `cargo-deny`, Gradle's
verification and the licence allowlist stay the gates. A bot PR is merged like any other.

## Consequences

- New dependencies: `minisign-verify` (app-io), `self-replace` (app-sync), `minisign` (xtask only), and `hyper`'s
  `client` feature. All are pure Rust; `deny.toml` reviews them.
- The signing key is the credential to guard: whoever holds it can ship code to every user who turns updates on. If
  it is lost or leaked, a new key needs a release that carries both keys first. Key rotation is a manual procedure in
  `packaging/README.md`.
- If GitHub drops ChaCha20 or X25519, update checks fail with a TLS error in state until app-tls tier 2 lands. An
  `#[ignore]` live test (`DAIL_IT_UPDATES=1`) watches for that.
- `.deb` and MSI installs need the user's admin rights, so dail never installs those silently. iOS and Android only
  notify.
- An agent can check for and download updates, but only the user can install them.
