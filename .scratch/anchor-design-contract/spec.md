# Anchor Design Contract

## Problem Statement

`anchor` needs a decision-complete contract for the full product surface: store initialization, secret CRUD, metadata-only editing, password rotation, JSON/CSV migration, TOTP handling, recipient management, sync, and vault control. The design must stay compatible with the standard password-store layout, Tomb-backed storage, and local Git recovery, while avoiding ambiguous behavior that could lose data or expose the primary secret.

## Solution

`anchor` will remain a Linux-only CLI password manager that keeps the password-store tree as the source of truth, wraps the vault in Tomb, and uses Git for local history and recovery. It will expose store initialization, secret CRUD, read/copy/generate, `meta` and `metaedit` for metadata-only access, `update` for first-line password replacement with explicit confirmation, `import` and `export` for file-based JSON/CSV migration, recipient management, sync, and TOTP-only commands centered on canonical `otpauth://` data. Import/export will preserve secret text, entry metadata, and canonical TOTP data where the format supports it, and collisions will fail closed unless the user explicitly chooses overwrite or rename.

## User Stories

1. As a Linux user, I want to create, read, update, and delete secrets from the terminal, so that I can manage credentials without a GUI dependency.
2. As a Linux user, I want the vault to stay inside Tomb, so that stored filenames, modification times, and encrypted sizes are hidden at rest.
3. As a multi-device user, I want Git to be mandatory locally, so that every change is recoverable through history.
4. As a user initializing a new vault, I want `init` to create the store, Git repository, Tomb container, and recipient metadata, so that first-run setup is reproducible.
5. As a user with TOTP-based 2FA, I want TOTP secrets and codes handled by the same app, so that I do not need a separate authenticator workflow.
6. As a user editing URLs, usernames, and notes, I want `meta` and `metaedit`, so that I can change metadata without exposing the primary secret.
7. As a user rotating a password, I want `update` to show the current secret and require confirmation, so that I do not replace a secret accidentally.
8. As a user rotating many passwords, I want `update` to work across a path, directory, or glob, so that I can do batch replacements safely.
9. As a user editing multi-line entries, I want multiline update mode to be explicit, so that the default workflow stays first-line only.
10. As a user importing a legacy export, I want `import` to accept JSON and CSV files, so that I can migrate into `anchor` without manual recreation.
11. As a user exporting to another tool, I want `export` to write JSON or CSV, so that I can migrate out of `anchor` using common formats.
12. As a user migrating data, I want the file extension to select the format and mismatches to fail, so that I do not silently parse the wrong file type.
13. As a user with existing entries, I want import collisions to fail closed by default, so that migrations do not overwrite data unexpectedly.
14. As a user who wants an explicit override, I want overwrite or rename to be deliberate, so that collision handling is clear.
15. As a user storing TOTP data, I want canonical `otpauth://` storage and export, so that my data stays interoperable and recoverable.
16. As a user viewing TOTP data, I want URI validation and QR or clipboard output, so that enrollment and recovery are practical.
17. As a user managing recipients, I want add, remove, and list commands, so that I can control who can decrypt the vault.
18. As a user syncing machines, I want local changes to be refused when Git state is unsafe, so that I do not lose work.
19. As a user recovering from device loss, I want the vault to be reconstructable from encrypted files, Git history, and GPG backup, so that the system remains resilient.
20. As a user with a closed vault, I want commands to open and close Tomb automatically, so that normal operations stay safe by default.
21. As a user troubleshooting the vault, I want explicit vault commands, so that I can inspect state without guessing.
22. As a user on Linux, I want the app to stay CLI-only and daemonless, so that I can use it over SSH and in recovery contexts.

## Implementation Decisions

- Keep the public CLI centered on command handlers for `meta`, `metaedit`, `update`, `import`, `export`, `otp`, `vault`, and the existing CRUD and sync commands.
- Keep the public CLI centered on command handlers for `init`, `add`, `edit`, `remove`, `show`, `copy`, `generate`, `list`, `grep`, `meta`, `metaedit`, `update`, `import`, `export`, `otp`, `recipients`, `sync`, and `vault`.
- Treat the standard password-store layout as the on-disk source of truth and preserve entry metadata inside each encrypted entry.
- Keep metadata keys free-form and case-insensitive in lookup with ambiguity failure.
- Define `meta` and `metaedit` as metadata-only operations that never reveal the primary secret unless a user chooses a full secret-read command.
- Define `init` as the store bootstrap entrypoint that creates the password-store layout, Git repository, Tomb container, and initial recipient metadata.
- Define `add`, `edit`, `remove`, `show`, `copy`, `generate`, `list`, and `grep` as the baseline secret lifecycle and browse commands.
- Define `update` as a first-line replacement workflow that shows the current secret, asks for confirmation, and preserves the remainder of the entry unless multiline mode is explicit.
- Define `recipients` and `sync` as the recipient and Git recovery commands that enforce fail-closed mutation behavior on unsafe state.
- Define `import` and `export` as file-based commands where the file extension selects the format; invalid or mismatched formats fail.
- Restrict migration formats to JSON and CSV.
- Preserve secret text, metadata, and canonical TOTP data when the source or target format can represent them.
- Fail import collisions closed by default, with explicit overwrite or rename as the only escape hatches.
- Keep TOTP support canonicalized to `otpauth://` and exclude HOTP from the initial scope.
- Keep Tomb and Git mandatory, with remote sync optional, and preserve fail-closed behavior on unsafe Git state.
- Use explicit vault open, close, and status commands in addition to automatic lifecycle management during normal commands.
- Use the CLI boundary as the primary test seam; use narrower adapter seams only where format round-tripping or TOTP parsing needs direct proof.

## Testing Decisions

- Good tests assert external behavior at the CLI or adapter boundary, not internal implementation details.
- Test the highest seam possible first: command-level behavior for metadata-only access, password updates, import/export, TOTP rendering, and vault lifecycle.
- Add focused unit tests only for parsing and normalization edges that are hard to cover end to end, especially path sanitization, metadata lookup ambiguity, format selection, collision behavior, and TOTP URI normalization.
- Prior art is the existing acceptance-style design in `docs/product/design.md` and the ADRs; the implementation should follow those documented behaviors rather than invent new ones.

## Out of Scope

- HOTP counter-based flows.
- Apple Keychain import/export.
- GUI, browser extension, daemon, or server-hosted sync.
- New storage backends or databases.
- Multi-user or team-vault policy features.
- Non-Linux platforms.

## Further Notes

- The decision set is intentionally narrower than generic pass extensions: `meta` and `metaedit` instead of `tail`, JSON/CSV only for migration, and TOTP-only instead of a broader OTP subsystem.
- Import/export collision behavior is a user-visible policy, not an implementation detail.
- The CLI command surface should stay scriptable and deterministic.
