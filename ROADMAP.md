# dail — Roadmap

Each checkbox is roughly one agent-sized change. A box is ticked only when all gates in `AGENTS.md §6` are green.
Command names refer to the catalogue in [`ARCHITECTURE.md §8`](ARCHITECTURE.md#8-initial-command-catalogue-mvp).

Legend: **[core]** app-core · **[io]** app-io · **[sync]** app-sync · **[mcp]** · **[cli]** · **[gui]** · **[tk]** app-testkit

---

## Wave 1 — MVP: "a daily-drivable client for Hostinger + Gmail, fully agent-controllable"

**Exit criteria**
- One Hostinger and one Gmail (App Password) account work side by side.
- Sync, read, triage, compose/send and search all work offline-tolerant.
- Every catalogue command works via GUI/palette, `dail exec`, `dail batch` and MCP. The parity test is green.
- An agent attached via `dail mcp` to a running GUI can triage the inbox and draft replies. Sending needs user approval.
- A replay log from a real session reproduces the same state offline.

### M0 — Foundation
- [ ] Scaffold all crates (`[lints] workspace = true`, workspace deps), `xtask verify` running the gates from AGENTS.md §6
- [ ] [core] typed IDs, `Ctx`, `Origin`, `AppState` skeleton, `Engine::{dispatch, ingest}`, `Outcome`, `CommandReceipt`, `CoreError`
- [ ] [core] `CommandKind`, `CommandSpec`, `Command::specs()` (exhaustive), history stack with `undo`/`redo`
- [ ] [core] JSONL replay format + replay test harness; first fixture
- [ ] [tk] `MemoryBackend` (fake IMAP/SMTP), `MemoryStore`, fixture loader
- [ ] [sync] `Session` single-writer loop, `Backend` trait, effect executor skeleton, virtual clock for tests
- [ ] [cli] `dail exec`, `dail batch` (file/stdin, `--keep-going`, `--backend memory`), `dail schema`; `tests/smoke/basic.jsonl`
- [ ] [mcp] stdio server; tools generated from registry with kind annotations; registry ⇔ MCP ⇔ CLI parity test
- [ ] CI-ready: `cargo deny check` passes with `deny.toml`

### M1 — Accounts
- [ ] [core] `Account`, `ProviderPreset {hostinger, gmail, custom}`, `add_account` / `update_account` / `remove_account` (Destructive) / `test_account`
- [ ] [io] `config.toml` read/write (no secrets), profile dirs via `directories`
- [ ] [sync] keyring `StoreSecret` / `DeleteSecret`; `VerifyAccount` (IMAP LOGIN + SMTP AUTH, TLS via rustls/ring)
- [ ] [cli] alias `dail account add --preset gmail --email …` (password from prompt or `--password-stdin`, never argv)
- [ ] Live test (ignored): verify Hostinger + Gmail with `DAIL_IT_*` creds

### M2 — Sync (read path)
- [ ] [sync] `ListMailboxes` with SPECIAL-USE + preset mapping + name heuristics → `MailboxesListed`
- [ ] [sync] `FetchEnvelopes` (initial window N, incremental by `uid_next`, UIDVALIDITY reset) → `EnvelopesFetched`
- [ ] [io] `RedbStore` (tables, schema_version, migrations), content-addressed blobs; `PersistState` effect
- [ ] [core] threading (Message-ID / In-Reply-To / References, subject fallback) — pure + tests
- [ ] [core] `sync_now`, `list_mailboxes`, `list_threads`; poll `Tick` scheduling; `SyncStatus` in state
- [ ] [sync] flag resync for the cached window; lazy `FetchBody` + top-20 unread prefetch
- [ ] [io] MIME: text/plain + text/html → text (`html2text`), attachments list, charsets; `.eml` corpus snapshot tests

### M3 — Read & triage
- [ ] [core] View: `open_mailbox`, `select_threads`, `open_thread`, `close_thread`; Query: `get_thread`, `get_message`
- [ ] [core] `mark_read`/`mark_unread`, `star`/`unstar` with inverse + `PendingOp`
- [ ] [core] `archive_threads`, `trash_threads`, `move_threads` (preset-aware: Gmail All Mail, Hostinger Archive auto-create)
- [ ] [sync] `RunRemoteOp`: UID STORE / UID MOVE (COPY+EXPUNGE fallback), retries with back-off, reconcile on success/failure
- [ ] [core] undo cancels a still-pending op instead of issuing a compensating op; proptest undo round trip over random op sequences
- [ ] [core] `list_pending_ops`, `retry_pending_ops`, `discard_pending_op`; failures visible in state
- [ ] Offline test: triage with the backend down → ops queue → backend up → server state matches

### M4 — Compose & send
- [ ] [core] `Draft`; `create_draft`, `reply`, `reply_all`, `forward` (quoting, `In-Reply-To`/`References`, Re:/Fwd:), `update_draft`, `discard_draft`
- [ ] [core] `attach_file` / `remove_attachment` (path recorded, read by runtime); size limit check
- [ ] [io] `mail-builder` rendering of drafts (plain text, attachments, Message-ID from Ctx)
- [ ] [sync] Drafts: local-first, `AppendMessage` to Drafts on idle/close
- [ ] [core] `send_draft` (Outbound, `send_at = now + undo_send_ms`), `cancel_send`; tick-driven dispatch → `SmtpSend`
- [ ] [sync] SMTP send; APPEND to Sent unless the preset says the server saves it (Gmail); `SendFailed` → draft restored with error
- [ ] Virtual-clock tests: undo inside the window cancels; after the window it's irreversible

### M5 — Search
- [ ] [core] query language parser (`from: to: subject: is: has:attachment in: before: after: "…" -x`) + tests
- [ ] [core] `search` over the cached envelopes (Query), `set_search`/`clear_search` (View)
- [ ] [sync] server fallback: IMAP `UID SEARCH` translation; Gmail `X-GM-RAW` passthrough; merge results into cache

### M6 — GUI
- [ ] [gui] app shell: `Session` on a background tokio runtime, frame command queue, `request_repaint` on `StateChanged`
- [ ] [gui] three-pane layout (collapsible sidebar, virtualized thread list, reader), light/dark, one accent colour
- [ ] [gui] command palette (Ctrl+K): nucleo fuzzy over available specs + mailboxes + recent threads; second-step arg pickers
- [ ] [gui] keymap from `CommandSpec.default_keys` (`j/k e # s u r a f c / z g i`), shortcut hints in palette
- [ ] [gui] composer (chips, attachments via `rfd`, send with undo toast)
- [ ] [gui] account wizard (preset picker → email → password → test → sync)
- [ ] [gui] status bar: sync state, pending ops, errors, undo toast
- [ ] [gui] reader: text rendering of HTML, link targets shown, "open original" in browser
- [ ] [core] View/App commands `open_palette`, `close_palette`, `open_composer`, `focus_pane`, `set_setting`

### M7 — Agent surface
- [ ] [sync] single-instance lock + IPC listener (Unix socket / named pipe); `exec`/`batch`/`mcp` auto-attach
- [ ] [mcp] resources (`dail://mailboxes`, `dail://mailbox/{id}/threads`, `dail://thread/{id}`, `dail://state/view`)
- [ ] [mcp] `untrusted_content` envelopes everywhere; server instructions on prompt injection and approvals
- [ ] [core] `ApprovalRequest` for `Outbound` from `Origin::Mcp`; `approve_request`/`reject_request` (GUI/CLI origin only); `--allow-send`
- [ ] [gui] approvals tray (approve / edit / reject)
- [ ] Audit: replay log with origin, redacted bodies/secrets; `dail replay <log>` command
- [ ] E2E script: agent triages inbox and drafts a reply via MCP against the memory backend (CI)

---

## Wave 2 — Polish: "pleasant, fast, robust"

**Reading & rendering**
- [ ] HTML subset renderer in egui (paragraphs, headings, lists, blockquotes, links, tables-lite, inline images opt-in per sender)
- [ ] Conversation view: collapsed quoted text, per-message expand, signature folding
- [ ] Attachments: preview images/text/PDF-as-open-external, save / save-all, drag-out; drag-in to composer

**Sync & performance**
- [ ] IMAP IDLE push for INBOX (+ fallback polling), CONDSTORE/QRESYNC fast resync
- [ ] Full mailbox backfill in background with progress; configurable window per mailbox
- [ ] 100k-message mailboxes: virtualized list benchmarks, incremental threading, store compaction
- [ ] Full-text search index (`tantivy`, native only) over bodies; ranking + highlighting

**Productivity**
- [ ] Multi-select + bulk ops (shift/ctrl click, `x` select, select-all-matching)
- [ ] Contacts harvested from mail → recipient autocomplete
- [ ] Identities/aliases per account, signatures, default From per mailbox
- [ ] Unified inbox across accounts; per-account colour tag
- [ ] Palette: recents/frecency, arg flows for all commands, inline help from doc comments
- [ ] Configurable keymap (`config.toml` + `set_keybinding` command), vim/Gmail presets

**UX & platform**
- [ ] Onboarding: autoconfig (Mozilla ISPDB, `/.well-known/autoconfig`, SRV records) for unknown domains
- [ ] Desktop notifications (new mail, send failed, approval needed), optional tray icon
- [ ] Error & retry UX: actionable messages (auth failed → re-enter password; quota; TLS)
- [ ] Accessibility pass (AccessKit labels, focus order, high-contrast theme), font scaling, themes
- [ ] i18n scaffolding (UI strings table), RTL-safe layout
- [ ] Packaging: `.deb` + AppImage (Ubuntu), MSI (Windows) via `cargo-dist`; GitHub Actions matrix CI
- [ ] Crash-safe store with backups; `dail doctor` diagnostics command

**Agent ergonomics**
- [ ] MCP prompts (e.g. "triage inbox", "summarize thread") shipped as templates
- [ ] Streamable HTTP MCP transport (localhost, token-protected) in addition to stdio
- [ ] Rich receipts: diffs of changed entities for agents; `dry_run: true` flag for every Mutation

---

## Roadmap — future waves (unordered within themes)

**Accounts & protocols**
- OAuth2 XOAUTH2 for Gmail and Microsoft 365 (loopback redirect flow, token refresh in keyring)
- Native Gmail labels (`X-GM-LABELS`, `X-GM-THRID`); optional Gmail API backend
- JMAP backend (Fastmail, Stalwart) with push
- Exchange/M365 via Graph API

**Automation (ArtCraft dividends)**
- Rules engine: predicate + stored command batch, run on `EnvelopesFetched` (a server-side-filter alternative)
- Macros: record a slice of the command log, replay with parameters; bind to keys/palette
- Snooze, scheduled send, follow-up reminders (all `Ctx`-timed, deterministic)
- Templates/snippets with variables

**AI / agent features** (bring-your-own model through MCP; nothing hard-wired)
- Thread summaries, triage suggestions (archive/label/reply-needed) shown as reviewable proposals
- Reply drafting via MCP sampling; the user edits and sends
- Natural-language search → query language translation
- Agent-proposed rules ("you always archive X") produced as a reviewable rule draft

**Security & trust**
- OpenPGP sign/encrypt/verify (`pgp` crate, pure Rust); Autocrypt
- S/MIME
- DKIM/SPF/DMARC/ARC verdict display (`mail-auth`), lookalike-domain and phishing warnings
- One-click unsubscribe (RFC 8058 `List-Unsubscribe-Post`)
- At-rest encryption of the local cache (key in keyring)

**Ecosystem**
- ICS invites: parse, render, RSVP; CalDAV calendar view
- CardDAV contacts sync
- Import/export: mbox, Maildir, `.eml`; migrate from Thunderbird
- Plugin API over the command bus (out-of-process, JSON-RPC)

**Platforms**
- macOS build (Keychain via `keyring`)
- `dail serve`: headless daemon exposing the command bus over WebSocket JSON-RPC + HTTP MCP
- Web/PWA client for Android/iOS on top of `dail serve` (revisit egui-wasm vs a DOM UI such as Dioxus then; restore the wasm gate for `app-core`)
- Direct-JMAP serverless web mode
