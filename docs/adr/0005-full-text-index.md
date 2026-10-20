# 0005 — A local full-text index of downloaded bodies (`tantivy`)

- Status: accepted
- Date: 2026-10-19
- Plan: `ROADMAP.md`, Wave 2 (Sync & performance)

## Context

`search` matches the cached envelopes (headers, flags) and asks the servers for the rest (`SearchMailbox`), so a word
that is only in a body is found only while the account is online, and only as the server's search finds it (Hostinger's
Dovecot often does no full-text search at all). The bodies dail has downloaded are blobs in the store, which the engine
can't read (sans-IO), so the engine can't match them either.

Matching every blob on each search would read and parse every stored message, which is far too slow for a 100k-message
profile. An inverted index is needed. The only mature pure-Rust one is `tantivy`. Its default features include zstd
column compression, which compiles C (`zstd-sys`), and `AGENTS.md §5` forbids C in the build.

## Decision

- `tantivy` 0.27 in `app-io` (`fulltext`), with `default-features = false` and `mmap`, `lz4-compression`:
  pure Rust (`lz4_flex`, `memmap2`, `fs4`), MIT. `cargo deny` passes; 48 crates join the tree.
- The index is derived data, like the blobs: it lives in `<profile>/index`, is never the source of truth and can be
  thrown away. Documents are `{message, subject, from, body}`; only `message` (the `MessageId`) is stored.
- The runtime keeps it up to date where it already sees the data: a `FetchBody` that stores a body indexes it, a
  `PersistState` whose delta removes messages deletes them. Bodies stored before the index existed are indexed when the
  backend starts (in the background). The engine never touches the index.
- Searching stays a `Command`. `search`/`set_search` with text terms also emit `SearchIndex{search, text, limit}`; the
  runtime answers `IndexSearched{search, hits}` (or `IndexSearchFailed`). The engine keeps the hits per query in
  `status.index_searches` and treats them like server hits: a hit shows when its non-text terms match the cache.
- `app-testkit`'s memory backend uses tantivy's in-RAM directory, so the same code runs in tests.

## Consequences

- Body words are found offline and on servers without full-text search, in every downloaded message.
- Build time and binary size grow (tantivy and its dependencies). The index takes disk space (roughly a fifth of the
  bodies' text).
- An index can lag: a body fetched by another dail process attached over IPC is indexed by the process that owns the
  session, which is the only one that runs effects; a crash between storing a blob and indexing it leaves the body out
  until the next start re-indexes missing bodies.
- Ranking and highlighting (tantivy's BM25 scores, snippets) are a separate roadmap item; hits come back in score order
  already, and the list shows them newest first like every search.
