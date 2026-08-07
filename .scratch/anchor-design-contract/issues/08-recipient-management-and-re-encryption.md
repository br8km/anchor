# 08 — Recipient management and re-encryption

**What to build:** The end-to-end behavior that manages recipient keys and re-encrypts the vault safely during hardware rotation or device migration.

**Blocked by:** 01 — Store bootstrap and vault lifecycle

**Status:** ready-for-agent

- [ ] `anchor recipients add`, `anchor recipients remove`, and `anchor recipients list` work against the current vault recipient set.
- [ ] Re-encryption preserves decryptability for the remaining recipients and supports safe key rotation without losing vault access.

