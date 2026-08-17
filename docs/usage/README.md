# Anchor Usage Guide

`anchor` is a Linux-only CLI password manager built around the standard password-store layout, Tomb-backed vault storage, and local Git history.

## Core model

- The store root is the password-store tree, usually `~/.password-store`.
- The default store root comes from `ANCHOR_STORE` if it is set, otherwise `HOME/.password-store`.
- Secrets are encrypted as `.gpg` files.
- The first line of each entry is the primary secret.
- Remaining lines are free-form metadata stored inside the encrypted entry.
- TOTP data is stored inside entry metadata as a canonical `otpauth://` URI.

## Typical workflow

1. Initialize the vault with `anchor init --recipient <RECIPIENT>`.
2. Add or generate secrets with `anchor add` or `anchor generate`.
3. Read, copy, browse, or edit metadata with `anchor show`, `anchor copy`, `anchor list`, `anchor grep`, `anchor meta`, and `anchor metaedit`.
4. Rotate passwords with `anchor update`.
5. Store or read TOTP data with `anchor otp`.
6. Import or export JSON or CSV with `anchor import` and `anchor export`.
7. Manage recipients with `anchor recipients`.
8. Inspect or control Tomb directly with `anchor vault`.
9. Sync local Git state with `anchor sync`.

## Guide map

- [Getting Started](getting-started.md)
- [Secrets and Browsing](secrets.md)
- [Metadata, Updates, and TOTP](metadata-update-totp.md)
- [Migration, Recipients, and Sync](migration-recipients-sync.md)
- [Command Reference](command-reference.md)

## External requirements

`anchor` shells out to the system `gpg`, `git`, and `tomb` binaries, and clipboard copying uses `wl-copy` first with `xclip` as a fallback.
