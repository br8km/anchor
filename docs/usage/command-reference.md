# Command Reference

This is a compact reference for the current CLI surface.

## Global options

- `--store <PATH>` sets the vault root.
- `--clipboard-timeout-ms <MILLISECONDS>` sets the clipboard clear timeout used by clipboard-copy commands.

## Top-level commands

- `init`
- `vault`
- `add`
- `edit`
- `remove`
- `generate`
- `update`
- `show`
- `meta`
- `metaedit`
- `copy`
- `list`
- `grep`
- `import`
- `export`
- `sync`
- `otp`
- `recipients`

## `init`

```bash
anchor init --recipient alice@example.com
```

Creates the store, the local Git repository, the Tomb container, and the initial recipient metadata.

## `vault`

- `anchor vault open`
- `anchor vault close`
- `anchor vault status`

Use these for manual recovery and troubleshooting.

## Secret lifecycle

- `anchor add NAME`
- `anchor edit NAME`
- `anchor remove NAME`
- `anchor generate NAME`

`add` reads the full entry body from stdin. `edit` and `generate` preserve metadata unless you deliberately replace it.

## Read and browse

- `anchor show NAME`
- `anchor copy NAME`
- `anchor list`
- `anchor grep TERM`

## Metadata and password rotation

- `anchor meta NAME`
- `anchor metaedit NAME`
- `anchor update [PATH]...`

`anchor update` accepts one or more targets and supports `--multiline`.

## Migration

- `anchor import FILEPATH`
- `anchor export FILEPATH`

`import` supports `--overwrite` and `--rename`.

## TOTP

- `anchor otp add NAME`
- `anchor otp code NAME`
- `anchor otp uri NAME`
- `anchor otp validate URI`

`otp code` and `otp uri` support `--clipboard`.

## Recipients

- `anchor recipients add RECIPIENT`
- `anchor recipients remove RECIPIENT`
- `anchor recipients list`

## Sync

- `anchor sync`
- `anchor sync status`

## Exit behavior

The current binary exits with `0` on success and `1` for any error path.
