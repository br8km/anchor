# 01 — Store bootstrap and vault lifecycle

**What to build:** The end-to-end behavior that creates a new vault, sets up its local Git history, and keeps Tomb open and closed automatically for normal commands while still exposing explicit vault control.

**Blocked by:** None — can start immediately

**Status:** ready-for-agent

- [ ] `anchor init` creates the password-store layout, Git repository, Tomb container, and initial recipient metadata in a clean environment.
- [ ] `anchor vault open`, `anchor vault close`, and `anchor vault status` work as explicit commands, and mutating commands fail closed on unsafe Git state before they change the vault.

