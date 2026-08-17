# Getting Started

`anchor` is designed for a solo Linux user who wants CLI-only password management with local recovery through Git and Tomb.

## Prerequisites

- Linux.
- `gpg`.
- `git`.
- `tomb`.
- A GPG recipient that can decrypt the vault.
- Optional clipboard support through `wl-copy` or `xclip`.

## Default store location

If you do not pass `--store`, `anchor` uses:

- `ANCHOR_STORE` when that environment variable is set.
- Otherwise `HOME/.password-store`.

## First-time setup

Initialize the vault with at least one recipient:

```bash
anchor init --recipient alice@example.com
```

You can pass `--recipient` more than once if you want the initial recipient set written into the vault metadata:

```bash
anchor init --recipient alice@example.com --recipient bob@example.com
```

What `init` creates:

- The password-store directory tree.
- A local Git repository.
- A Tomb container for the vault.
- The initial recipient metadata in `.gpg-id`.

## What to expect after initialization

- `anchor` keeps the vault closed until a command needs it.
- Mutating commands open Tomb automatically, do the work, commit the change, and close Tomb again unless it was already open.
- Mutating commands fail closed if the Git working tree is dirty.

## Manual vault control

Use explicit vault commands when you need to troubleshoot or recover:

```bash
anchor vault status
anchor vault open
anchor vault close
```

## Sanity check

After initialization, you can confirm the vault layout and state with:

```bash
anchor vault status
anchor recipients list
anchor sync status
```
