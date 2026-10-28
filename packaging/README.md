# Packaging

- `linux/dail.desktop`, `linux/dail.svg`: the desktop entry and icon (`app_id` and `StartupWMClass` are `dail`, which
  is also the `desktop-entry` hint of dail's notifications).
- Debian/Ubuntu: `cargo deb -p app-cli` (with [cargo-deb](https://crates.io/crates/cargo-deb) installed) builds
  `target/debian/dail_<version>-1_<arch>.deb` from `[package.metadata.deb]` in `crates/app-cli/Cargo.toml`.
- Release builds: `scripts/release-windows.ps1` makes `dist/dail-<version>-x86_64-pc-windows-msvc.zip` and
  `dist/dail-<version>-x86_64.msi` (WiX v5, `windows/dail.wxs`); `scripts/release-ubuntu.sh` makes
  `dist/dail-<version>-x86_64-unknown-linux-gnu.tar.gz` (binary, desktop entry, icon), the `.deb` (needs cargo-deb)
  and `dist/dail-<version>-x86_64.AppImage` (`linux/AppRun`; appimagetool 1.9.0 is downloaded and checked by
  sha256; it runs `desktop-file-validate`, from `desktop-file-utils`), each with a `.sha256`. The `release` workflow
  (`.github/workflows/release.yml`) runs on a pushed `vX.Y.Z` tag, after a guard job checks the tag against every
  version site (`cargo xtask release check-tag`); started by hand from the Actions tab it is a dry run that uploads
  `dist/` as run artifacts only. On a tag, the `publish` job (environment `release`, the only job with
  `contents: write`) signs the manifest and runs `gh release create` with every artifact and `.sha256`, the notes
  being the tag's `CHANGELOG.md` section (`cargo xtask release notes`).
- `windows/dail.manifest`: embedded into `dail.exe` by `crates/app-cli/build.rs` (MSVC targets). Its
  `consoleAllocationPolicy` of `detached` stops Windows 11 24H2+ from opening a console when dail is started from
  Explorer, the Start menu or a shortcut, so only the GUI appears. Run from a terminal, dail still uses that terminal.
- Versions: the version is written in one place, `[workspace.package]` in `Cargo.toml`. Gradle reads it,
  `cargo xtask ios` passes it to xcodebuild as `MARKETING_VERSION`, the apps get it from the binary, and the mobile
  contract fixtures write `<version>`. `cargo xtask release bump <x.y.z>` rewrites it, refreshes `Cargo.lock` and
  adds a `CHANGELOG.md` section (`--note <text>` fills it, else a stub).
  `cargo xtask release versions` lists the sites; `cargo xtask release next <patch|minor|major>` prints the next
  version.
- Release on merge (MISC-9): `.github/workflows/bump.yml` runs after `ci` passes on a push to `main` (a merged PR;
  `workflow_run`), so a merge that breaks `main` is not released, and `ci` skips the bump's own commit. It bumps the
  patch version, or the minor / major one if the PR has the label `release:minor` / `release:major` (`release:skip`
  releases nothing), with the PR title as the changelog line, commits `workspace: release vX.Y.Z` to `main` and pushes
  the tag, which starts `release.yml`. It pushes with a GitHub App token, because pushes made with `GITHUB_TOKEN` start
  no workflows. One-time setup: create a GitHub App owned by the repository owner with *Contents: read and write*,
  install it on the repository, and store its numeric ID as the variable `RELEASE_APP_ID` and its private key as
  the secret `RELEASE_APP_PRIVATE_KEY`, both in the `release` environment (which also needs `main` in its deployment
  branches; no required reviewers: the merge is the approval). Only admins and the App can push `v*` tags
  (`.github/rulesets/release-tags.json`), so only a merge, through `bump.yml`, starts a release that publishes. Create the labels `release:minor`, `release:major` and `release:skip`.
- Update manifest (ADR 0011): `cargo xtask release manifest [--dist dist]` writes `dist/manifest.json` (target, kind,
  URL, sha256 and size of each installable artifact) and `dist/manifest.json.minisig`, signed with the minisign key in
  `DAIL_UPDATE_SIGNING_KEY` (the text of the secret key file) and `DAIL_UPDATE_SIGNING_PASSWORD`. The trusted comment
  is `version:<x.y.z>`.
- Signing key: `DAIL_UPDATE_SIGNING_PASSWORD=… cargo xtask release keygen <dir>` makes an encrypted pair. The secret key
  and its password go into the `release` environment's secrets only, never into the repository; keep an offline copy.
  The public key (last line of `dail-update.pub`) is in `UPDATE_PUBLIC_KEYS` in `crates/app-io/src/update.rs`.
  Rotation: ship a release whose binary trusts both the old and the new public key, signed with the old key; from the
  next release on, sign with the new key and drop the old one. A leaked key is rotated the same way, at once.
