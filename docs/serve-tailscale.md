# A dail server on your tailnet

`dail serve` (ADR 0017, `ARCHITECTURE.md` §5.6) can serve your devices over [Tailscale](https://tailscale.com). The
connection is TLS with a pinned certificate and a device token either way; Tailscale only decides who can reach the
port.

## On the server

1. Find the machine's tailnet address and MagicDNS name: `tailscale ip -4` (e.g. `100.101.102.103`) and
   `tailscale status --json` (its `Self.DNSName`, e.g. `home.tail1234.ts.net`).
2. Run the server on that address, advertising the name:

   ```sh
   dail serve --listen 100.101.102.103:7443 --advertise home.tail1234.ts.net
   ```

   or put these flags into `ExecStart` of the systemd user unit (`packaging/systemd/README.md`). The address is in
   `100.64.0.0/10`, so dail does not warn about it. Listening on `0.0.0.0` works too, but then the port is open on
   every network the machine is on; prefer the tailnet address. Add `--listen 127.0.0.1:7443` as well if local
   tools should reach it without Tailscale.
3. Note the fingerprint it prints at start (`dail serve: certificate SHA-256 fingerprint …`, also in
   `journalctl --user -u dail-serve`).
4. Open a pairing code for the device:

   ```sh
   dail serve pair --role user --name Laptop
   ```

   It prints the code (single use, 10 minutes), the fingerprint, and the `dail remote add` line for each address.
   Use `--role agent` for a machine that runs an agent: it may only submit as MCP, under the approvals.

## On the device

A device profile has no accounts. Use a new profile for it:

```sh
dail --profile home remote add home.tail1234.ts.net:7443
```

dail shows the fingerprint the server presents. Compare it with the one from step 3; only if they are equal, answer
yes and type the code. For scripts: `--fingerprint <hex> --code <CODE>` (a different fingerprint is refused).
`dail --profile home remote status` shows the link.

The profile is now a remote profile: `[remote]` in its `config.toml` names the server, the pinned fingerprint and the
keyring entry of the device token (the token itself is in the OS keyring). Every dail on it connects to the server and
mirrors its mail: `dail --profile home tui`, `dail --profile home gui`, `dail --profile home exec …`, and
`dail --profile home mcp` for an agent. Commands that change mail run on the server; what you look at (the open
mailbox, the theme) stays on the device. If the server can't be reached, dail retries in the background and
`dail --profile home remote status` shows the error. While the link is down you keep working on the mirror: triage,
drafts and sends are applied at once and queued (they survive a restart of dail), then sent to the server in order
when it is back; a command the server refuses then is kept aside (`remote status` counts it; `retry_offline` or
`dismiss_offline_rejection` deal with it). What can't wait or must not be decided on a stale mirror (emptying the
trash, deleting folders, accounts and certificates, pairing and revoking devices, approvals, `sync_now`) is refused
offline with `ERR014` and nothing changes. The newest bodies (200, INBOX first) are kept on the device after each
connection, so they open offline.

## Firewall

- With `--listen <tailnet address>`, only peers on your tailnet reach the port. Tailscale ACLs can narrow that to
  your own devices (`"dst": ["home:7443"]`).
- If the machine runs a host firewall (`ufw`), allow the port on the Tailscale interface only:
  `sudo ufw allow in on tailscale0 to any port 7443 proto tcp`.
- Do not forward the port on your router. If you must listen on a public address, dail warns at start; TLS, the
  device tokens and the rate-limited pairing codes are then all that protect it.

## Losing a device

`dail serve devices` lists the paired devices, `dail serve revoke <id>` revokes one: its token stops working and its
connections close at once.
