# 0015 — Settings and sorting rules stored on the mail server

- Status: proposed
- Date: 2026-11-04
- Plan: `ROADMAP.md`, Wave 12

## Context

Settings live on each device (`config.toml`, `state.redb`). A user with the desktop, the TUI and a phone sets the theme,
preferences and sorting three times, and nothing sorts mail the same way everywhere. dail has no cloud of its own and
must not get one (local-first, AGENTS.md §0). The only server every user already has is the mail server.

What the reference providers offer (probed 2026-11-04):

| Mechanism | Gmail | Hostinger |
|---|---|---|
| IMAP folder (APPEND, FETCH, IDLE) | yes, as a label | yes |
| IMAP METADATA (RFC 5464) | no | not in pre-login capabilities; post-login unknown |
| ManageSieve (RFC 5804, port 4190) | no | no answer on port 4190 |
| Provider filter API | Gmail API, needs OAuth2 | none public |

## Decision

- **Store.** Each account has a folder `dail/Settings` (created on first write, hidden from the mailbox list). It holds
  messages whose single `application/json` part is a `SyncedSettings` document. The message with the highest UID is
  current; a writer APPENDs a new one and then marks the older ones `\Deleted` and expunges them in that folder only.
  This uses only IMAP that dail already speaks, so it works on every server.
- **Content.** `SyncedSettings { schema, updated_ms, theme, prefs, rules }`: the theme by built-in name or as custom
  tokens (ADR 0008), the preferences that are not device-specific, and sorting rules. Window sizes, paths, keymaps per
  device and anything secret stay local. The schema is public API like `config.toml` (AGENTS.md §5): versioned, unknown
  fields ignored, and a golden document under `crates/app-io/testdata/golden/`.
- **Merge.** Each field and each rule carries `updated_ms` from `Ctx`. Pull merges field by field and rule by `RuleId`,
  newest wins; deletes are tombstones kept 90 days. The merge is a pure function in `app-core` with property tests.
  Which account holds the settings is chosen once per profile (default: the first account).
- **Rules run in dail.** A rule is a condition (from, to, cc, subject, `List-Id`, has attachment, mailbox; AND/OR/NOT)
  and actions (move, add label, mark read, star, archive). The engine evaluates them on `EnvelopesFetched` as a pure
  function and dispatches ordinary commands with `Origin::Rule`, so they are undoable and logged. Actions are
  state-based, so two devices running the same rule do the same thing once.
- **No forwarding action.** A rule that sends mail off the machine without the user would bypass the agent policy
  (AGENTS.md §4), so rules cannot forward or reply. Agents may propose and save rules (`save_rule` is `agents: allowed`);
  `push_settings` and choosing the settings account are `UserOnly`.
- **Server-side execution is an exporter, not the store.** Where a `ProviderPreset` says ManageSieve exists, the rules
  that Sieve can express are compiled to one script `dail` and uploaded; the rest stay client-only and are shown as
  such. Gmail filters through the Gmail API wait for Gmail OAuth2.
- **No METADATA.** The hidden folder is the only store, even on servers that offer IMAP METADATA: one code path, one
  format, and the same behaviour on every provider.
- **Untrusted input.** Anyone with the password, or another client, can write that folder. A document over 256 KiB, or
  one that fails validation, is ignored with a visible error; it never carries secrets, and rule text is never
  interpreted as instructions.

## Consequences

Settings follow the user across devices with no new service and no new dependency class. Sorting happens only while some
dail instance is running, until a Sieve or Gmail exporter covers the rule. Other clients see a `dail/Settings` folder
(Gmail: a label) with a few small messages. A new `Origin::Rule` and the `SyncedSettings` schema become public API.
Conflicting offline edits of the same field lose the older one.
