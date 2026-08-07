# Use the standard password-store layout

`anchor` keeps the standard password-store file layout as the canonical on-disk format, with Tomb as the Linux at-rest wrapper and Git as the sync/recovery layer. We chose this over a native app-owned format because it preserves interoperability, simplifies recovery, and keeps the app's app-specific metadata small and explicit instead of forcing users into a new storage scheme.
