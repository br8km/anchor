# 03 — Secret read and browse

**What to build:** The end-to-end behavior that reads secrets, copies them, and browses the vault without exposing more plaintext than the user asked for.

**Blocked by:** 01 — Store bootstrap and vault lifecycle

**Status:** implemented

- [x] `anchor show`, `anchor copy`, `anchor list`, and `anchor grep` work against encrypted entries and the password-store tree.
- [x] Copy-to-clipboard clears the clipboard after the configured timeout, and read-only output never includes extra plaintext beyond the requested secret or field.
