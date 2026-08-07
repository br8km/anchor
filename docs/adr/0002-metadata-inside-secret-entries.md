# Store app-specific metadata inside secret entries

`anchor` stores app-specific metadata as explicit key/value lines inside each encrypted secret entry rather than in a separate metadata store. We chose this because it keeps the storage model simple, preserves the standard password-store layout as the source of truth, and keeps metadata easy to inspect, export, and recover alongside the secret itself.
