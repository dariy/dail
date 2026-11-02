# 0013 — Stable error codes

- Status: accepted
- Date: 2026-11-01
- Plan: `ROADMAP.md`, Wave 9, MISC-35 (`docs/requests/misc.md`)

## Context

Error messages are translated, so a user cannot quote one and an agent or a search cannot match on it. The `error` slugs
in `CoreError` JSON are public API, and AGENTS.md §5 asks for an alias or an ADR to rename one.

## Decision

- Each error has a code of its own, separate from command names. The ranges are: 001–099 `app-core`, 100–199 `app-io`,
  200–299 `app-sync`, 300–399 `app-host`, 400–499 the front-ends and `app-mcp`.
- Messages start with `ERR<code>: `. Failures cross the engine as strings, so a code in the text survives translation
  of the sentence around it. `app-front` keeps the code when it translates a `CoreError`.
- `CoreError` slugs become `error_<code>_<slug>`, such as `error_008_origin_not_allowed`. No alias is kept: nothing
  uses the old slugs yet.
- A code is never changed or reused. A wrapper of a coded error is `transparent`, so a message has one code.
- A test in `app-testkit` scans every `#[error]` for a unique code. Parser details (`app-core` search) and test
  fixtures are exempt.

## Consequences

Old slugs in existing logs no longer deserialize as errors. A new error does not pass the tests without a code. Failures
made outside the error enums, such as a server's text or the host's `{"error": "…"}` for a null argument, have no code.
