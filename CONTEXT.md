# anchor

This context defines the project glossary for `anchor`, a Linux-only CLI password manager for a solo user with multiple devices. It keeps the domain terms precise and excludes implementation details.

## Language

**anchor**:
The Linux-only CLI password manager for a solo user with multiple devices.
_Avoid_: app, tool, password manager

**vault**:
The encrypted password store and its Tomb container on Linux.
_Avoid_: store, locker, repository

**recipient**:
A GPG key authorized to decrypt vault contents.
_Avoid_: key, keypair, user

**password-store layout**:
The standard file-per-secret encrypted tree format used for compatibility and recovery.
_Avoid_: custom format, native format, tree

**entry metadata**:
Explicit key/value lines stored inside an encrypted secret entry after the primary secret line.
_Avoid_: tags, properties, fields

**metadata key**:
A user-chosen key name in an entry metadata line.
_Avoid_: label, name, field name

**metadata lookup**:
A case-insensitive match against metadata keys that fails when multiple keys differ only by case.
_Avoid_: exact lookup, strict lookup

**metadata-only access**:
Viewing or editing only the metadata lines after the primary secret line.
_Avoid_: partial edit, secret-free edit, tail

**password update workflow**:
A command path that replaces the first line of an existing entry while preserving the rest of the entry by default.
_Avoid_: rotation, rewrite

**sync conflict**:
A case where two devices changed the same secret offline and the repository cannot be synced automatically.
_Avoid_: merge conflict, sync error

**totp canonical form**:
An `otpauth://` URI used as the normalized representation of a TOTP secret.
_Avoid_: seed, raw OTP, QR payload, HOTP
