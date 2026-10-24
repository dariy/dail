# dail

A local-first, keyboard-driven email client that agents can drive. It is written in Rust.

Every action, from archiving a thread to moving the selection, is a serializable `Command`. You can send the same
command from the GUI or its command palette (Ctrl+K), the terminal UI, the CLI (`dail exec`), a JSONL batch script,
or the built-in MCP server (`dail mcp`). One deterministic engine runs them all. Email content is treated as untrusted
input. An agent cannot send mail without the user's approval, and it cannot reach the few user-only commands at all.

## Platforms

- **Ubuntu** (X11/Wayland) and **Windows**: `egui` desktop GUI, `ratatui` terminal UI (`dail tui`), CLI and MCP.
- **Android 10+**: a Kotlin/Compose app over the same Rust engine (ADR 0009).
- **iOS/iPadOS**: in progress (ROADMAP Wave 5).

Accounts: generic IMAP + SMTP with a password or app password. Hostinger Mail and Gmail are the reference providers.
Passwords are kept only in the OS keyring.

## Build

You need Rust ≥ 1.92 with `rustfmt` and `clippy`, plus `cargo-deny`. `rust-toolchain.toml` pins the channel.

```sh
cargo xtask verify                      # fmt, deny, clippy, tests, headless smoke test
cargo run -p app-cli -- gui             # the desktop GUI
cargo run -p app-cli -- gui --backend memory   # the GUI over a demo mailbox, no network
cargo run -p app-cli -- schema          # every command, as JSON Schema
```

The Android build is `cargo xtask android all`. AGENTS.md §6 lists what it needs.

## Releases

The `release` workflow (Actions → release → Run workflow) builds a Windows zip, a Linux tar.gz and `.deb`, and an
APK per Android ABI, each with a `.sha256`. They are uploaded as the run's artifacts. `packaging/README.md` has the
details.

## Documentation

- [`AGENTS.md`](AGENTS.md): the rules for contributors and coding agents.
- [`ARCHITECTURE.md`](ARCHITECTURE.md): how the system works, including the command catalogue.
- [`ROADMAP.md`](ROADMAP.md): what is done and what comes next.
- [`docs/adr/`](docs/adr/): the architecture decisions.

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
The bundled fonts have their own licences (`crates/app-theme/assets/fonts/`).

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in dail by you, as
defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
