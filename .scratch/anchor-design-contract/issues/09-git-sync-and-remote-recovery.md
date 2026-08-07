# 09 — Git sync and remote recovery

**What to build:** The end-to-end behavior that syncs the vault through Git while staying safe on dirty or divergent state and still supporting optional remotes.

**Blocked by:** 01 — Store bootstrap and vault lifecycle

**Status:** ready-for-agent

- [ ] `anchor sync` and `anchor sync status` work against the local Git repository and optional remote configuration.
- [ ] Sync pulls before mutation, commits local changes, pushes when a remote exists, and fails closed on dirty or divergent state.

