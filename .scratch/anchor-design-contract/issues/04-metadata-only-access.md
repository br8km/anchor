# 04 — Metadata-only access

**What to build:** The end-to-end behavior that lets a user inspect or edit everything after the first secret line without revealing that first line.

**Blocked by:** 02 — Secret write and generation, 03 — Secret read and browse

**Status:** ready-for-agent

- [ ] `anchor meta` hides the first line while still showing the metadata lines for an entry.
- [ ] `anchor metaedit` preserves the first line, supports editing the remaining metadata, and fails on ambiguous metadata keys that differ only by case.

