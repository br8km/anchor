# anchor

`anchor` is a Linux-only CLI password manager built on the standard password-store layout, Tomb-backed vault storage, and local Git history.

## What it does

- Stores secrets as encrypted `.gpg` files.
- Keeps the first line as the primary secret and the rest as free-form metadata.
- Supports metadata-only edits, password rotation, TOTP, import/export, recipient management, and Git sync.
- Opens and closes Tomb automatically for normal commands, with explicit `vault` commands for manual control.

## Requirements

- Linux
- `gpg`
- `git`
- `tomb`
- Optional: `wl-copy` or `xclip` for clipboard support

## Quick start

```bash
anchor init --recipient alice@example.com
anchor add services/email
anchor show services/email
```

By default, `anchor` uses `ANCHOR_STORE` if set, otherwise `HOME/.password-store`.

## Usage

- [Usage guide](docs/usage/README.md)
- [Getting started](docs/usage/getting-started.md)
- [Secrets and browsing](docs/usage/secrets.md)
- [Metadata, updates, and TOTP](docs/usage/metadata-update-totp.md)
- [Migration, recipients, and sync](docs/usage/migration-recipients-sync.md)
- [Command reference](docs/usage/command-reference.md)
