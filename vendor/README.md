# vendor/

Third-party crates that dail patches. Excluded from the workspace (`[workspace] exclude`), so dail's lints, rustfmt
and `version.workspace` rules do not apply to them; each keeps its upstream licence files.

## hayro-postscript (ADR 0012)

- **Source:** https://github.com/LaurenzV/hayro, directory `hayro-postscript/`, at commit
  `5a4ddbb861e97d7693400c4d8d97363caa2c884f` (the commit `hayro` is pinned to in `Cargo.toml`).
- **Patch:** `src/object.rs`, `read` skips runs of `<<` and `>>` in a loop. Upstream calls itself once per delimiter,
  so a CMap with a long run of them (a few hundred thousand bytes) overflows the stack and aborts the process. The fix
  follows pdfcraft's (`vendor/README.md` there, storytold/craft-libs#263). Regression tests:
  `long_open_dict_run_does_not_overflow` and `long_close_dict_run_does_not_overflow`. The manifest inlines the
  upstream workspace's fields and drops the examples.
- **Wired in by:** `[patch."https://github.com/LaurenzV/hayro"] hayro-postscript = { path = "vendor/hayro-postscript" }`.
- **Drop it when:** a hayro release on crates.io reads the delimiters without recursion. Then move `hayro` back to
  crates.io, delete this directory, the `[patch]` entry and `allow-git` in `deny.toml`.
