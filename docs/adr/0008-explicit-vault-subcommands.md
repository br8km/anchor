# Keep explicit vault subcommands

`anchor` exposes `vault open`, `vault close`, and `vault status` as first-class commands even though ordinary operations open and close Tomb automatically. We chose this because explicit vault control improves recovery, troubleshooting, and operator confidence without changing the default safe behavior for normal commands.
