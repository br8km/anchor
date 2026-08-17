# Migration, Recipients, and Sync

This page covers the commands that move data in and out of the vault, rotate recipients, and sync the local Git repository.

## Import

`anchor import` reads a JSON or CSV file and writes entries into the current store.

```bash
anchor import backup.json
anchor import legacy.csv
```

Format selection:

- The file extension selects the format.
- Invalid or mismatched formats fail.
- Supported formats are JSON and CSV.

Collision policy:

- Import collisions fail closed by default.
- `--overwrite` is an explicit override.
- `--rename` is the other explicit override.
- `--overwrite` and `--rename` cannot be used together.

What is preserved:

- Secret text.
- Entry metadata.
- Canonical TOTP data where the source format can represent it.

## Export

`anchor export` writes the current vault to JSON or CSV.

```bash
anchor export backup.json
anchor export backup.csv
```

Format selection works the same way as import: the destination extension selects JSON or CSV, and mismatches fail.

## Recipients

`anchor recipients` manages the GPG recipients that can decrypt the vault.

List current recipients:

```bash
anchor recipients list
```

Add a recipient:

```bash
anchor recipients add bob@example.com
```

Remove a recipient:

```bash
anchor recipients remove alice@example.com
```

Operational behavior:

- Adding or removing a recipient re-encrypts the vault for the updated recipient set.
- The `.gpg-id` metadata file is updated as part of the change.
- Removing the last recipient is rejected.

## Sync

`anchor sync` keeps the local Git repository in sync with the configured remote, if one exists.

```bash
anchor sync
anchor sync status
```

`sync status` reports:

- The current Git branch.
- Whether the worktree is clean.
- Whether a remote is configured.

`sync` behavior:

- It pulls first when the remote branch exists.
- It pushes local commits when a remote is configured.
- It fails closed on dirty or otherwise unsafe Git state.
- If no remote is configured, it reports that explicitly and does not invent a remote.

## Practical rotation flow

1. Add the new recipient.
2. Confirm the new machine can decrypt the vault.
3. Remove the old recipient once the transition is complete.
4. Run `anchor sync` if you want the Git remote updated too.
