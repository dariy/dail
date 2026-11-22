# Contributing

Read [`AGENTS.md`](AGENTS.md) first. Its rules apply to people as well as coding agents. Then read
[`ARCHITECTURE.md`](ARCHITECTURE.md).

- **Scope.** Keep a change to about one `ROADMAP.md` item. A new command follows the sequence in AGENTS.md §3.
- **Gates.** `cargo xtask verify` must pass: fmt, deny, clippy `-D warnings`, the tests and the headless smoke test
  (AGENTS.md §6). If you change Android code, run `cargo xtask verify android` as well.
- **Docs.** Code and docs change in the same commit. Tick the roadmap item, and update the command catalogue when you
  add or rename a command.
- **Disk space.** Each change of features or flags leaves another copy of the affected crates in `target/`, which grows
  by tens of GB a week. `scripts/sweep-target.sh` removes the units cargo has not used for 7 days (`--days N`,
  `--dry-run`; `--cross` also drops the Android and iOS targets, and `--all` runs `cargo clean`).
- **Commits.** `<crate>: <imperative summary>`, e.g. `app-core: add archive_threads command`.
- **Dependencies.** Pure Rust only, added through `[workspace.dependencies]`. `deny.toml` enforces this.
- **Licence.** Contributions are dual licensed under MIT OR Apache-2.0 (see [`README.md`](README.md#licence)).
