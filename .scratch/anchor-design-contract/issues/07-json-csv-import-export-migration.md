# 07 — JSON/CSV import-export migration

**What to build:** The end-to-end behavior that imports and exports vault data through JSON and CSV files with reversible mappings where the format supports them.

**Blocked by:** 02 — Secret write and generation, 03 — Secret read and browse, 06 — TOTP entry lifecycle

**Status:** ready-for-agent

- [ ] `anchor import` and `anchor export` select the format from the file extension and fail on invalid or mismatched formats.
- [ ] Import and export preserve secret text, entry metadata, and canonical TOTP data where supported, and collisions fail closed unless the user explicitly chooses overwrite or rename.

