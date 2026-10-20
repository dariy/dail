# Wycheproof test vectors

Unmodified copies of files from Project Wycheproof, <https://github.com/C2SP/wycheproof>, directory `testvectors_v1/`,
at commit `7a325932231bd7a724c791c8b0d65ea4d6d2f435` (fetched on 2026-10-18). License: Apache-2.0.

The unit tests of `app-tls` read them (`src/testutil.rs`): every `valid` case must verify or decrypt, every `invalid`
case must fail, and `acceptable` cases are rejected (`docs/plan-tls.md` §7.1).

To update, fetch the same file names at a newer commit and change the commit above.
