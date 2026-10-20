# Packaging

- `linux/dail.desktop`, `linux/dail.svg`: the desktop entry and icon (`app_id` and `StartupWMClass` are `dail`, which
  is also the `desktop-entry` hint of dail's notifications).
- Debian/Ubuntu: `cargo deb -p app-cli` (with [cargo-deb](https://crates.io/crates/cargo-deb) installed) builds
  `target/debian/dail_<version>-1_<arch>.deb` from `[package.metadata.deb]` in `crates/app-cli/Cargo.toml`.
