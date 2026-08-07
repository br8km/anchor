# 05 — Password update workflow

**What to build:** The end-to-end behavior that updates existing passwords safely, including targeted replacement across one entry, a directory, or a glob.

**Blocked by:** 02 — Secret write and generation, 03 — Secret read and browse

**Status:** ready-for-agent

- [ ] `anchor update` shows the current secret, asks for confirmation, and replaces the first line by default.
- [ ] `anchor update` supports directory and glob targeting, with explicit multiline update mode when the user wants to replace more than the first line.

