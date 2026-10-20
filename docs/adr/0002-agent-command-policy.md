# 0002 — Agents may not run every command: a per-command policy in the registry

- Status: accepted
- Date: 2026-10-18

## Context

`AGENTS.md §1` asks for channel parity: every `Command` can be triggered identically from the GUI, the CLI, batch
scripts and MCP. Until now the only exception was the outbound policy of `AGENTS.md §4` (an agent's `send_draft` becomes
an approval request; `approve_request`/`reject_request` check `Origin::is_user` in their handlers).

The code review of 2026-10-17 (`review.md`) found commands that are harmless for the user but let an agent move data or
credentials off the machine without a send and without approval:

- `update_account` from `Origin::Mcp` could point a verified account at another IMAP host. The next `sync_now` or poll
  sends the keyring password there.
- `restore_draft` takes a whole `Draft` as its argument. An agent could forge an attachment check
  (`ready{size, content_type}` for any path) and header fields (a `Message-ID` with CR/LF). The idle draft save would
  then read the file and `APPEND` it to the Drafts mailbox.
- `add_account` (found while fixing the two above) lets an agent add an account on a server it controls. Drafts saved
  for that account, with their attachments, go to that server.

The checks lived in the handlers, so the boundary was invisible and easy to forget for the next command.

## Decision

- Every command has one `AgentAccess` in the registry: `CommandName::agent_access()`, an exhaustive `match` like
  `kind()`, also exposed as `CommandSpec.agents`.
  - `allowed`: agents run it like the user.
  - `approval`: agents run it, but their sends wait for the user (`send_draft`; `dail mcp --allow-send` lifts it).
  - `user_only`: only the user's origins (`gui`, `cli`, `batch`; `Origin::is_user`) may run it.
- `user_only`: `add_account`, `update_account`, `restore_draft`, `approve_request`, `reject_request`.
- `Engine::dispatch` checks the policy for every command a front-end submits. A refused command returns
  `origin_not_allowed` and changes nothing. The per-handler checks are removed.
- Commands that `undo`/`redo` apply come from the engine's own history (an inverse the engine built, or a command that
  passed the check when it was first dispatched), so they are not checked again. An agent can still undo the user's
  `discard_draft` (which applies `restore_draft`) or the user's account edit.
- MCP keeps listing every tool. Parity means every command is reachable and answered the same way on every surface. The
  doc comment (the MCP description) of each `user_only` command says that agents get `origin_not_allowed`.
- `remove_account` stays `allowed`: it is `Destructive`, needs `confirm: true` and carries MCP's `destructiveHint`, as
  `AGENTS.md §3` defines for that kind. It cannot move data off the machine.

## Consequences

- The boundary is in one table. `app-core/tests/it/registry.rs` pins the `user_only` and `approval` sets, checks that
  `approval` is exactly the `Outbound` kind, and checks that every `user_only` description names the error.
  `app-core/tests/it/agents.rs` dispatches every `user_only` example from both MCP origins.
- A new command must choose its `AgentAccess`. The exhaustive `match` does not compile until it does.
- The policy binds the MCP surface. A process that can run `dail exec` as the user (or connect to the profile socket
  and claim `cli`) acts as the user. `ARCHITECTURE.md §9` says so.
- Agents can no longer set up accounts. The agent end-to-end test has the user add the fake server's account through
  `dail account add`, attached to the running `dail mcp` over IPC.

## Later additions

- Wave 2: `save_attachments` is `user_only`. It moves no data off the machine, but it writes files under names and into
  folders the caller chooses; an agent could plant a file where something runs it (an autostart entry, a shell profile).
- Wave 2: `set_identity` and `remove_identity` are `user_only`: the addresses mail goes out `From` are account
  configuration, like `update_account`. Choosing among them (`set_mailbox_identity`, `set_draft_from`) is `allowed`; an
  agent's send still waits for the user, whose card shows the `From`.
- Wave 2: `set_password` is `user_only`, like `add_account`: it chooses the credential dail logs in with.
