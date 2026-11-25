# Packaging

- `linux/dail.desktop`, `linux/dail.svg`: the desktop entry and icon (`app_id` and `StartupWMClass` are `dail`, which
  is also the `desktop-entry` hint of dail's notifications).
- `systemd/dail-serve.service`: `dail serve` (ADR 0017) as a systemd user unit, with install, logs, the keyring on a
  headless machine and the hardening in `systemd/README.md`; running it over Tailscale is `docs/serve-tailscale.md`.
- Icons (MISC-39): `linux/dail.svg` is the source. `cargo xtask icons` renders `icons/dail-<n>.png` (16–512 px; the
  desktop window embeds the 256 px one), `windows/dail.ico` (the MSI's `ARPPRODUCTICON`), `windows/dail.res` (the icon
  resource `app-cli/build.rs` links into `dail.exe`) and the iOS `AppIcon` (`ios/Dail/Assets.xcassets`, 1024 px, no
  alpha). They are committed; the build never renders them. `.github/workflows/icons.yml` reruns the task and commits
  the result when the SVG changes.
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
- MSI and `PATH` (DSK-3): `windows/dail.wxs` appends `%ProgramFiles%\dail` to the system `PATH` (removed on
  uninstall), so `dail mcp`, `dail exec` and `dail tui` work in a new terminal and in an agent's MCP configuration.
- winget (DIST-3): the package `dariy.dail` (`winget install dariy.dail`) installs the MSI per machine. On a tag, the
  `winget` job of `release.yml` runs `scripts/release-winget.sh update <x.y.z>`, which opens the pull request on
  `microsoft/winget-pkgs` with [komac](https://github.com/russellbanks/Komac) (pinned by sha256; it reads the MSI's
  hash and ProductCode from the published release, and carries the package text over from the previous version).
  One-time setup: a classic PAT with `public_repo` from the `dariy` account (komac forks `winget-pkgs` there if
  needed) as the secret `WINGET_TOKEN` in the `release` environment, then the first version by hand:
  `GITHUB_TOKEN=<pat> scripts/release-winget.sh new` in a terminal (it holds the package text; answer *Commands*
  with `dail`, *Tags* with `email mail imap smtp mcp tui cli`, and Enter elsewhere). Until that version is merged into
  `winget-pkgs`, or without the secret, the job logs a notice and passes. `DRY_RUN=1` prints the manifests only.
  The MSI is not signed yet (DSK-4); winget accepts that, but SmartScreen and the validation's Defender scan may
  delay a first review. An MSI installed by winget keeps its own updater (ADR 0011); winget sees the newer version.
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
