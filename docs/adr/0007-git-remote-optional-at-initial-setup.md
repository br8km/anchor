# Allow a local Git repository before configuring a remote

`anchor` creates and uses a local Git repository during initial setup, but it does not require a remote to be configured on day one. We chose this because it keeps first-run setup usable offline, lets the user defer account or network decisions, and still preserves local history for recovery and later synchronization.
