# 0001 — `app-cli` may depend on `app-testkit` for `--backend memory`

- Status: accepted
- Date: 2026-10-16

## Context

`AGENTS.md §2` allows `app-testkit` only as a `[dev-dependency]`. `ARCHITECTURE.md §5.1` and `§7.2`, however, require the
shipped `dail` binary to run fully offline with `--backend memory [--fixture <name>]`, against the `MemoryBackend` and
fixtures of `app-testkit`. Gate 6 of the Verification Loop (`dail batch --backend memory tests/smoke/basic.jsonl`) depends
on this, and so do demos and agent end-to-end scripts (M7).

Alternatives considered:

1. Move `MemoryBackend`, `MemoryStore` and the fixtures into `app-sync` / `app-io`. Keeps the rule, but puts test fakes
   and fault injection into production crates that every front-end links, and splits the test kit in two.
2. Put the memory backend behind a cargo feature of `app-cli`. Gate 6 and the docs would then depend on a non-default
   build, and `cargo test --workspace` would not cover it by default.

## Decision

`app-cli` (the `dail` binary) may depend on `app-testkit` as a normal dependency. No other crate may. `app-testkit` stays
free of network code and secrets, so linking it adds no new capability beyond an in-memory fake.

## Consequences

- `AGENTS.md §2` lists the exception next to the `app-testkit` rule.
- `app-testkit` is held to library standards (no panics, no `println!`) like every other library crate, which it already is.
- Fixtures are compiled into the binary; they must stay small and contain no real personal data.
