# Packaging

- `linux/dail.desktop`, `linux/dail.svg`: the desktop entry and icon (`app_id` and `StartupWMClass` are `dail`, which
  is also the `desktop-entry` hint of dail's notifications).
- Debian/Ubuntu: `cargo deb -p app-cli` (with [cargo-deb](https://crates.io/crates/cargo-deb) installed) builds
  `target/debian/dail_<version>-1_<arch>.deb` from `[package.metadata.deb]` in `crates/app-cli/Cargo.toml`.
- Release builds: `scripts/release-windows.ps1` makes `dist/dail-<version>-x86_64-pc-windows-msvc.zip` and
  `dist/dail-<version>-x86_64.msi` (WiX v5, `windows/dail.wxs`); `scripts/release-ubuntu.sh` makes
  `dist/dail-<version>-x86_64-unknown-linux-gnu.tar.gz` (binary, desktop entry, icon), the `.deb` (needs cargo-deb)
  and `dist/dail-<version>-x86_64.AppImage` (`linux/AppRun`; appimagetool 1.9.0 is downloaded and checked by
  sha256), each with a `.sha256`. The `release` workflow
  (`.github/workflows/release.yml`) runs on a pushed `vX.Y.Z` tag, after a guard job checks the tag against every
  version site (`cargo xtask release check-tag`); started by hand from the Actions tab it is a dry run that uploads
  `dist/` as run artifacts only.
- `windows/dail.manifest`: embedded into `dail.exe` by `crates/app-cli/build.rs` (MSVC targets). Its
  `consoleAllocationPolicy` of `detached` stops Windows 11 24H2+ from opening a console when dail is started from
  Explorer, the Start menu or a shortcut, so only the GUI appears. Run from a terminal, dail still uses that terminal.
- Versions: `cargo xtask release bump <x.y.z>` writes the version into every site (`[workspace.package]` in
  `Cargo.toml`, which Gradle reads, and `MARKETING_VERSION` in `ios/project.yml`), refreshes `Cargo.lock` and the
  mobile contract fixtures, and adds a `CHANGELOG.md` stub. `cargo xtask release versions` lists the sites.
- Update manifest (ADR 0011): `cargo xtask release manifest [--dist dist]` writes `dist/manifest.json` (target, kind,
  URL, sha256 and size of each installable artifact) and `dist/manifest.json.minisig`, signed with the minisign key in
  `DAIL_UPDATE_SIGNING_KEY` (the text of the secret key file) and `DAIL_UPDATE_SIGNING_PASSWORD`. The trusted comment
  is `version:<x.y.z>`.
- Signing key: `DAIL_UPDATE_SIGNING_PASSWORD=… cargo xtask release keygen <dir>` makes an encrypted pair. The secret key
  and its password go into the `release` environment's secrets only, never into the repository; keep an offline copy.
  The public key (last line of `dail-update.pub`) is `UPDATE_PUBLIC_KEY` in `crates/app-io/src/update.rs`.
  Rotation: ship a release whose binary trusts both the old and the new public key, signed with the old key; from the
  next release on, sign with the new key and drop the old one. A leaked key is rotated the same way, at once.
