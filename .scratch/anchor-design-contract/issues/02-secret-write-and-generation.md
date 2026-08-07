# 02 — Secret write and generation

**What to build:** The end-to-end behavior that creates, edits, removes, and generates secrets while preserving the entry format and avoiding plaintext leakage.

**Blocked by:** 01 — Store bootstrap and vault lifecycle

**Status:** ready-for-agent

- [ ] `anchor add`, `anchor edit`, `anchor remove`, and `anchor generate` work against real entries in the password-store layout.
- [ ] Generated and edited secrets stay in the first line by default, preserve existing metadata when appropriate, and never leak plaintext in logs or command-line arguments.

