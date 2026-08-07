# anchor Password Manager Design

---

## Objective
Build `anchor`, a Linux-only password manager in Rust that keeps secrets recoverable, auditable, and easy to sync across machines without introducing a server-side dependency. The product must be CLI-first, compatible with the standard password-store layout, and must treat Tomb-backed vault storage, Git history/sync, TOTP support, metadata-only secret access, password update workflows, and file-based import/export support for JSON and CSV that round-trips secret text, entry metadata, and canonical TOTP data where the format supports it, with collision-safe defaults, as required parts of the core design rather than optional add-ons.

The primary business value is to give a solo user or small personal setup a password workflow that stays usable offline, survives device loss through Git and recipient backups, and reduces metadata leakage by storing the password-store layout inside a Tomb container on Linux.

## User Stories
- As a Linux user, I want to create, read, update, and delete secrets from a terminal so that I can manage passwords on any machine without a GUI dependency.
- As a Linux user, I want the vault to live inside Tomb so that filenames, modification times, and encrypted sizes are hidden when the vault is closed.
- As a multi-machine user, I want Git to be mandatory locally so that every change is versioned and can be synced to a remote backup when I choose.
- As a user with TOTP-based 2FA accounts, I want TOTP secrets and TOTP codes handled by the same app so that I do not need a separate authenticator workflow for stored credentials.
- As a user who inspects or edits metadata, I want to view and change everything except the first-line secret so that I can work on URLs, usernames, and notes without exposing the password.
- As a user rotating passwords, I want an update workflow that can target one entry or many entries so that I can replace secrets efficiently without re-entering unrelated data.
- As a user coming from another password manager, I want first-class import and export flows so that I can migrate into `anchor` and back out again using common file formats.
- As a user who rotates hardware, I want recipient management and re-encryption so that I can add a new device, migrate encryption keys, and retire an old key safely.
- As a user coming from another password-store tool, I want compatible password-store layout and command behavior so that my existing habits and recovery options continue to work.

## User Flows

### First-time setup
The user installs the app and its Linux dependencies, creates or selects a recipient, initializes the store, initializes Git, and creates the Tomb container for the store root. The app verifies the vault is available, writes the initial recipient metadata, and records the vault as a local Git repository. A remote can be added later.

### Add or edit a secret
The user opens the vault implicitly through a command such as `add` or `edit`. The app ensures the Git working tree is ready, loads the current recipient set, lets the user enter plaintext through the editor or stdin, encrypts the result to `.gpg`, writes the file into the password-store layout, commits the change, and pushes if a remote is configured.

### Inspect or edit metadata only
The user opens an existing entry in a metadata-only mode such as `meta` or `metaedit`. The app decrypts the entry, hides the first line from display, and lets the user inspect or edit the remaining metadata without exposing the password itself.

### Show or copy a secret
The user asks for a secret by name, the app decrypts the entry, selects the first line or requested property, and prints it or places it into the clipboard with a short timeout. The plaintext must never be logged, written to shell history, or embedded in a command-line argument.

### Generate a secret
The user requests a new password for an entry. The app creates a strong random secret, stores it as the first line, optionally preserves additional metadata, and can copy the result to the clipboard or open the entry in the editor for post-generation edits.

### Update existing passwords
The user runs an update command against one entry, a directory, or a glob, and the app shows the current secret before replacing it. The user can generate a new password, provide one manually, or open the entry in the editor, and the app preserves the existing metadata unless the user explicitly chooses a multiline update.

### Add or use TOTP
The user stores an `otpauth://` URI or equivalent TOTP seed information in an entry. The app can render the current TOTP code, copy it to the clipboard, validate the URI, or display a QR code or provisioning URI for enrollment in another authenticator app.

### Import existing data
The user imports secrets from a JSON or CSV export file into the existing password-store layout. The file extension selects the format, invalid or mismatched formats fail, and the app maps the source data into the password-store layout, preserves secret text, metadata, and canonical TOTP data where the file format supports it, fails closed on collisions unless the user explicitly overrides, and otherwise keeps the migration behavior deterministic.

### Export existing data
The user exports secrets from the password store to a JSON or CSV file. The file extension selects the format, invalid or mismatched formats fail, and the app reads the existing password-store layout and writes a file that round-trips secret text, metadata, and canonical TOTP data where the format supports it.

### Sync across machines
The user pulls before mutation, makes a local change, commits the result, and pushes if a remote is configured. A new machine clones the Git repository from the configured remote, imports or generates its recipient, and re-encrypts the vault so the new device can read the secrets it is authorized to decrypt.

### Rotate a recipient
The user adds a new recipient, re-encrypts the vault to include both old and new recipients during transition, confirms that the new machine can decrypt the vault, and then removes the old recipient once the migration is complete.

## Acceptance Criteria
- Given a Linux machine with `gpg`, `git`, and `tomb`, when the user initializes the app, then the vault, Git repository, Tomb container, and recipient metadata are created successfully.
- Given an existing secret, when the user requests it by name, then the app decrypts only the requested entry and does not expose plaintext outside the intended output path.
- Given an existing secret, when the user opens it in metadata-only mode, then the app hides the first line and allows the user to inspect or edit the remaining lines without exposing the password.
- Given a generated password entry, when the user selects copy-to-clipboard, then the first line is copied and the clipboard is cleared after the configured timeout.
- Given an existing password entry, when the user runs an update workflow, then the app shows the current secret, asks for confirmation, and can replace the first line while preserving the rest of the entry unless the user explicitly requests a multiline replacement.
- Given a TOTP-enabled secret, when the user asks for a current TOTP code, then the app produces a valid time-based one-time password locally without contacting a remote service.
- Given a TOTP entry, when the user asks for its provisioning URI or QR code, then the app emits the canonical `otpauth://` form without changing the stored secret.
- Given an import file, when the user runs a migration, then the app imports JSON or CSV data into the existing password-store layout, preserves secret text, metadata, and canonical TOTP data, and handles duplicates explicitly instead of guessing.
- Given a password-store layout, when the user exports to a file, then the app can write JSON or CSV output that round-trips secret text, metadata, and canonical TOTP data without exposing plaintext in logs or arguments.
- Given a recipient rotation, when the user re-encrypts the store, then the new recipient can decrypt subsequent content and the old recipient can be removed after the transition.
- Given a dirty or divergent Git repository, when the user runs a mutating command, then the app stops with a clear error instead of silently overwriting or discarding changes.
- Given a closed Tomb, when the user accesses the store, then the app opens the Tomb automatically for the duration of the command and closes it again afterward.

## Non-Goals
- macOS, Windows, iOS, and Android support.
- A web UI, browser extension, or background daemon.
- Team-vault features such as multi-user sharing, access policies, approvals, or audit logs.
- Server-hosted synchronization, hosted accounts, or vendor-managed encryption.
- HOTP counter-based OTP flows.
- A custom cryptography implementation in Rust when the system `gpg` binary can provide the OpenPGP boundary.
- A database-backed secret store; the password-store layout remains the source of truth.
- Mandatory cloud storage integration or any dependency on a specific cloud provider.

---

## Tech Stack
The application is a Rust workspace with `resolver = "3"`, a core library crate, and one CLI binary crate. The user-facing product is terminal-only, and the runtime dependencies are the system `gpg`, `git`, and `tomb` binaries on Linux.

Recommended supporting libraries and tools:
- Rust stable, current edition for the repo.
- Cargo workspace with resolver 3 for the core domain, CLI orchestration, and tests.
- `gpg` as the OpenPGP backend instead of embedding a separate crypto engine.
- A TOTP library for local OTP parsing and code generation.
- A clipboard backend for Linux desktops, with Wayland and X11 support where available.
- The configured `$EDITOR` for plaintext editing.

The app should not require a persistent daemon. Each command should be self-contained so the tool works cleanly over SSH, in local shells, and in recovery situations.

## Architecture Decisions

### 1. Store format
The store is a directory of encrypted `.gpg` files with a root `.gpg-id` file that records the recipient set. This keeps the product compatible with standard password-store recovery and tooling while preserving the simple file-per-secret mental model.

### 2. Tomb is mandatory
The Linux store must live inside a Tomb container. Tomb is not an optional hardening feature; it is part of the product promise. The app should open the Tomb on demand for a command, keep it mounted only as long as needed, and close it again after the operation completes.

### 3. Git is mandatory locally
Every store is a Git repository. Local history is always present, and remote synchronization is optional and only active when a remote is configured. The app should fail closed on dirty or divergent state rather than guessing at a merge strategy.

### 4. TOTP is mandatory
TOTP support is part of the core product, not an extension. The app stores the TOTP seed or provisioning URI inside the same secret entry model and provides local code generation, URI validation and export, and clipboard copy commands. HOTP counter-based flows are out of scope for the initial design.

### 5. Metadata-only access is explicit
The app exposes a metadata-only view and edit path so users can inspect or change everything after the first line without revealing the primary secret.

### 6. Password updates are a first-class workflow
The app exposes an update path for replacing existing passwords in place, including support for targeted selection, generate/provide/edit modes, and preserving metadata by default.

### 7. Import and export are first-class migration workflows
The app exposes dedicated import and export paths for migrating to and from JSON and CSV file formats, with round-tripping limited to secret text, entry metadata, and canonical TOTP data where the format supports it, and duplicate handling where appropriate.

### 8. CLI-only UI
The user interface is terminal-only. No GUI, tray app, or background service is required for the core workflow. This keeps the implementation small, testable, and recoverable over SSH.

### 9. Secure plaintext handling
Plaintext should be held in memory only as long as necessary. Temporary editor files should prefer secure temporary locations when possible, clipboard contents should expire automatically, and logs must never contain secret values.

### 10. Compatibility over novelty
The design should prioritize compatibility with the standard password-store file model and simple operational recovery over speculative features. Nested recipient files, multi-tenant support, and advanced policy engines are out of scope.

## Data Model

| Entity | Purpose | Key Fields | Persistence |
| --- | --- | --- | --- |
| StoreRoot | Canonical location of the password store | absolute path, current branch, tomb mountpoint | filesystem |
| SecretEntry | One encrypted secret file | relative path, ciphertext path, display name, last modified time | `.gpg` file |
| RecipientSet | The recipients allowed to decrypt the vault | normalized GPG fingerprints, recipient metadata | `.gpg-id` and exported public keys if used |
| TotpSecret | TOTP material attached to an entry | issuer, account label, secret/URI, digits, period, algorithm | inside the encrypted entry |
| GitState | Repository status for sync operations | clean/dirty, branch, remote, upstream, last commit | `.git` metadata |
| TombState | Container state for the store | tomb file path, key file path, mounted/unmounted, mountpoint | tomb files plus mount state |

### Secret layout
Each secret entry is a text payload encrypted into a `.gpg` file. The first line is the primary password or shared secret. Additional lines store free-form key/value metadata as plain text fields.
Metadata keys are preserved exactly as written, but user-facing lookups are case-insensitive and must fail when two keys differ only by case and create ambiguity.

### Naming rules
Secret names are path-like and may contain subfolders, but the app must reject traversal, empty segments, and other unsafe names. The display name is derived from the path relative to the store root.

### OTP layout
OTP data lives with the secret instead of in a separate database. The app should accept either a raw seed or a provisioning URI and normalize both into an `otpauth://` URI for storage and export, while still rendering current codes locally.

## API Contracts
The CLI is the public API. Commands should be stable, explicit, and designed for scripting.

| Command group | Example | Behavior |
| --- | --- | --- |
| Store init | `anchor init` | Create the vault root, initialize Git, write recipient metadata, and prepare the Tomb-backed vault. |
| Secret CRUD | `anchor add NAME`, `anchor edit NAME`, `anchor remove NAME` | Create or mutate a secret entry with editor or stdin support. |
| Metadata-only view | `anchor meta NAME`, `anchor metaedit NAME` | Inspect or edit entry metadata without displaying the first-line secret. |
| Secret read | `anchor show NAME`, `anchor copy NAME` | Decrypt a secret, print it, or copy it to the clipboard with timeout handling. |
| Password generation | `anchor generate NAME` | Generate a random password and write it as the first line of the entry. |
| Password update | `anchor update NAME`, `anchor update PATH...` | Replace existing passwords with generate/provide/edit modes, directory and glob targeting, and optional multiline updates. |
| Import/export | `anchor import FILEPATH`, `anchor export FILEPATH` | Import or export supported JSON and CSV file formats against the existing password-store layout. |
| Search and browse | `anchor list`, `anchor grep TERM` | List secret names or search decrypted content. |
| TOTP | `anchor otp add NAME`, `anchor otp code NAME`, `anchor otp uri NAME`, `anchor otp validate URI` | Store TOTP material from a seed or provisioning URI, validate URIs, display the current code, or print the canonical provisioning URI. |
| Sync | `anchor sync`, `anchor sync status` | Pull, commit, and push the Git repository according to current configuration. |
| Recipients | `anchor recipients add`, `anchor recipients remove`, `anchor recipients list` | Manage the recipients that can decrypt the vault. |
| Vault | `anchor vault open`, `anchor vault close`, `anchor vault status` | Control the Tomb-backed container directly for recovery, troubleshooting, and manual control. |

### Exit behavior
- `0` means success.
- `1` means a user-facing validation or usage error.
- `2` means crypto or key material could not be used.
- `3` means Git state is dirty, divergent, or otherwise unsafe for mutation.
- `4` means the Tomb or mount state is unavailable.
- `5` means clipboard integration failed after the secret was otherwise handled correctly.

### Output rules
- Secret values must never appear in debug output, commit messages, or error text.
- Read-only commands may print names, metadata, and user-requested fields, but never additional plaintext.
- Mutating commands should print a short deterministic summary suitable for logs and shell scripts.

## Integration Points
- `gpg` for encrypting and decrypting entries.
- `git` for local history and optional remote sync.
- `tomb` for Linux vault management and metadata hiding.
- `$EDITOR` for plaintext editing.
- Clipboard utilities for Linux desktop environments, with support for both Wayland and X11 where available.
- The filesystem itself, including secure temporary directories and permissions.

## Constraints & Boundaries
- Linux is the only supported platform.
- The product is CLI-only.
- Tomb is required, Git is required, and OTP is required.
- The app must remain recoverable from raw encrypted files, a GPG private key backup, and the Git history.
- The app should assume the user controls the local machine and, when configured, the remote Git host, but it should not assume the remote host is trusted with plaintext.
- The app should not rely on permanent background services or hidden state that cannot be reconstructed from the store.
- The app should expose explicit `vault open`, `vault close`, and `vault status` commands in addition to automatic vault management for normal operations.
- The app should not introduce a separate database or server component unless a separate approved requirement explicitly introduces it.
- The app should preserve interoperability with standard password-store tooling as long as the required Tomb wrapper is not in the way of plain-file inspection.

## Test Plan

### Unit tests
- Validate path normalization and rejection of unsafe names.
- Validate plaintext parsing, first-line extraction, and metadata field lookup.
- Validate metadata-only viewing and editing leaves the first line intact.
- Validate recipient normalization and comparison behavior.
- Validate password update target selection, first-line replacement, and multiline handling.
- Validate import/export path cleaning, duplicate handling, and dry-run behavior.
- Validate TOTP parsing, URI validation, and code formatting.
- Validate clipboard timeout and restoration logic.

### Integration tests
- Initialize a store in a temporary directory and verify the Git repository, recipient metadata, and Tomb structure.
- Add, edit, show, copy, generate, remove, and grep secrets end to end.
- Exercise metadata-only reads and edits end to end.
- Exercise password updates for one entry, a directory, and a glob.
- Import from at least one representative JSON or CSV source and verify path mapping and duplicate handling.
- Export to at least one representative JSON or CSV file and verify the output shape.
- Add TOTP material and verify current code generation, URI round-tripping, and QR or clipboard output.
- Rotate recipients and verify that re-encryption preserves decryptability for the new key.

### Recovery tests
- Restore from a Git clone and verify the app can re-open the store.
- Restore from encrypted files plus a GPG private key backup and verify the app can read secrets again.
- Verify that a closed Tomb does not expose the store contents until the vault is opened again.

### Security tests
- Confirm plaintext is not written to logs or command-line arguments.
- Confirm clipboard contents are cleared after the timeout.
- Confirm temp-file fallback uses safe permissions and is removed after use.
- Confirm the app fails closed on dirty or divergent Git state before mutation.

### Acceptance tests
- Exercise the full initial setup path on a clean Linux machine.
- Exercise a metadata-only edit flow and confirm the secret line is never displayed.
- Exercise a password update flow that preserves metadata and replaces the first line only.
- Exercise an import path from a representative legacy export into the standard store layout.
- Exercise a multi-machine sync path with one machine generating a new recipient and another machine re-encrypting the store.
- Exercise a TOTP enrollment and code-copy flow with a real test account.
