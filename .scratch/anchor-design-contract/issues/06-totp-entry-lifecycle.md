# 06 — TOTP entry lifecycle

**What to build:** The end-to-end behavior that stores, validates, displays, and exports canonical TOTP data.

**Blocked by:** 02 — Secret write and generation, 03 — Secret read and browse

**Status:** implemented

- [x] `anchor otp add`, `anchor otp code`, `anchor otp uri`, and `anchor otp validate` work against canonical `otpauth://` data.
- [x] TOTP codes are generated locally, QR or clipboard output works, and HOTP counter-based flows are not part of the supported behavior.
