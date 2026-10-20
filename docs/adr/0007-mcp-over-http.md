# 0007 — MCP over Streamable HTTP on loopback

- Status: accepted
- Date: 2026-10-20
- Plan: `ROADMAP.md`, Wave 2 (agent ergonomics)

## Context

`dail mcp` speaks MCP over stdio: the client starts it as a child process. Some clients (and agents running in another
process tree, a container, or a long-lived assistant) connect to a running MCP server over HTTP instead. MCP's
Streamable HTTP transport is that; rmcp implements it as a tower service. Serving it means running an HTTP server, a
new kind of dependency and a new way in: until now only the user's own processes reached dail, through stdio or the
profile's socket.

## Decision

- `dail mcp --http <addr>` serves rmcp's `StreamableHttpService` with `hyper` 1 (HTTP/1.1 server) and `hyper-util`
  (pure Rust; `http-body-util` and `bytes` were already in the tree via rmcp). Path `/mcp`.
- Loopback only: an address that is not loopback is refused before binding. rmcp checks the `Host` header against
  `localhost`, `127.0.0.1` and `::1` (DNS rebinding).
- Every request needs `Authorization: Bearer <token>` (compared in constant time). The token is 32 random bytes from
  the OS (`getrandom`), hex, kept in `<profile>/mcp-token` with user-only permissions, so the same token works across
  restarts and only the user's processes can read it. A private memory session (no profile) prints a one-off token.
  Browsers can't send the header to another origin without a CORS preflight, which is never answered.
- Everything else is the stdio server: same tools, resources, prompts and instructions; calls run as `Origin::Mcp`
  (`--allow-send`: `McpAllowSend`), so the agent policy and approvals apply unchanged.

## Consequences

- Clients that only speak HTTP can use dail; the token file is the credential to guard (delete it to rotate).
- Plain HTTP on loopback: other local users can't read the token, but a local process running as the user can — the
  same trust as the profile socket.
- No TLS and no remote access by design; a remote agent needs its own tunnel (e.g. SSH port forwarding).
