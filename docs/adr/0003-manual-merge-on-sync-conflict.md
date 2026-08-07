# Fail sync and require a manual merge on conflict

When two devices edit the same secret offline, `anchor` stops sync instead of choosing a winner or creating parallel versions. We chose this because it keeps secret recovery explicit, avoids silent data loss, and forces the user to resolve the conflict with full context before the repository is synced again.
