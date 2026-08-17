# Secrets and Browsing

These commands operate on entries in the password-store tree. Secret names are path-like, so names such as `personal/email` and `work/vpn` are valid.

## Add a secret

`anchor add` reads the full entry body from standard input and stores it as a new encrypted entry.

```bash
printf 'first-secret\nurl=https://example.test\nnotes=keep\n' | anchor add services/email
```

Rules:

- The first line must contain the secret value.
- Existing entries are not overwritten.
- The rest of the entry is preserved as metadata lines inside the encrypted file.

## Edit a secret

`anchor edit` replaces only the first line of an existing entry and keeps the remaining metadata.

```bash
printf 'new-secret\n' | anchor edit services/email
```

Use this when you want to update the primary password but keep the same metadata.

## Generate a secret

`anchor generate` creates a new random 32-character alphanumeric secret and stores it as the first line.

```bash
anchor generate services/email
```

If the entry already exists, only the first line is replaced and the metadata is preserved.

## Remove a secret

`anchor remove` deletes the encrypted entry.

```bash
anchor remove services/email
```

## Read a secret

`anchor show` prints only the first line of the entry.

```bash
anchor show services/email
```

`anchor copy` copies the first line to the clipboard and clears the clipboard after the configured timeout.

```bash
anchor copy services/email
```

The clipboard timeout defaults to 10 seconds and can be changed with `--clipboard-timeout-ms`.

## Browse entries

`anchor list` prints all stored secret names.

```bash
anchor list
```

`anchor grep` searches decrypted entry contents and prints matching names.

```bash
anchor grep email
```

## Naming rules

- Names may contain `/` to form subdirectories.
- Unsafe names are rejected.
- The stored file uses a `.gpg` suffix for the leaf entry.
