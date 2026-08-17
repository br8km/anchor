# Metadata, Updates, and TOTP

This page covers the commands that operate on the non-secret part of an entry, plus the password update workflow and TOTP handling.

## Metadata-only access

`anchor meta` prints only the metadata lines after the first secret line.

```bash
anchor meta services/email
```

`anchor metaedit` replaces only the metadata section and preserves the first line.

```bash
printf 'url=https://new.example\nnotes=updated\n' | anchor metaedit services/email
```

Behavior to keep in mind:

- Metadata keys stay exactly as written in the file.
- Lookups are case-insensitive.
- If two keys differ only by case, metadata is treated as ambiguous and the edit fails closed.

## Password rotation

`anchor update` is the safe workflow for replacing passwords in place.

```bash
anchor update services/email
```

What it does:

- Shows the current secret before replacement.
- Prompts for confirmation.
- Replaces only the first line by default.
- Preserves the rest of the entry unless `--multiline` is used.

Target selection:

- You can pass one or more path arguments.
- The command can target a single entry, a directory, or a glob-style pattern.
- Multiple matches are deduplicated before the update runs.

Multiline mode:

```bash
anchor update --multiline services/email
```

Use multiline mode when you want to replace the whole decrypted body instead of only the first line.

## TOTP storage

`anchor` stores TOTP data inside the entry metadata under the `otp` key as a canonical `otpauth://` URI.

Add TOTP data from a raw seed or a URI on standard input:

```bash
printf 'JBSWY3DPEHPK3PXP\n' | anchor otp add services/email
```

For raw seeds, the entry name is used as the default label.

## TOTP reading

`anchor otp code` prints the current code.

```bash
anchor otp code services/email
```

Add `--clipboard` if you want the code copied to the clipboard as well:

```bash
anchor otp code --clipboard services/email
```

`anchor otp uri` prints the canonical URI stored in the entry metadata.

```bash
anchor otp uri services/email
```

`--clipboard` also works here:

```bash
anchor otp uri --clipboard services/email
```

## TOTP validation

`anchor otp validate` checks whether a URI is a valid supported TOTP URI.

```bash
anchor otp validate 'otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP'
```

Supported TOTP rules:

- Only TOTP is supported.
- HOTP counter-based flows are rejected.
- The canonical stored form is `otpauth://totp/...`.
- `SHA1` is the supported hash algorithm.
