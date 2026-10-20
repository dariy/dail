# Test certificates

Throwaway keys and certificates for the in-memory handshake tests of `app-tls` (`src/tests.rs`). **For tests only:**
the server keys are public, so these certificates prove nothing outside this directory.

- `root-ecdsa.pem` (P-384) signs `server-ecdsa.pem` (P-256, SHA-384), like Gmail's chain.
- `root-rsa.pem` (RSA 3072) signs `server-rsa.pem` (RSA 2048, SHA-256), like Hostinger's chain.
- Both server certificates are for `mail.dail.test` and valid until 2126. The keys are PKCS#8.

They were made with the script in `docs/plan-tls.md` Appendix B (OpenSSL 3.6, 2026-10-18). The root keys were
discarded; to change a certificate, run the script again and replace all six files.
