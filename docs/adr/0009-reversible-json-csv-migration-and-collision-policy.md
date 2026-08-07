# Keep JSON and CSV migration reversible and collision-safe

`anchor` treats JSON and CSV import/export as a reversible migration boundary for the data it understands: secret text, entry metadata, and canonical TOTP data. The file extension selects the format, and mismatched or invalid formats fail. When an imported item would collide with an existing entry, `anchor` fails closed unless the user explicitly requests overwrite or rename. We chose this because migration and backup should not silently lose data, and because predictable failure is easier to recover from than implicit renaming.
