# Agent Instructions

## Tooling

This project uses `mise` as the task runner and tool version manager.

Before running Rust, Node, pnpm, npm, cargo, test, lint, build, or project
maintenance commands, check `mise.toml` and prefer the existing mise tasks.

Examples:

- `mise tasks`
- `VERSION=x.y.z mise run kaleido:update`
- `mise run test`
- `mise run lint`
- `mise run ui-next:typecheck`

If a task exists for the workflow, use that task instead of spelling out the
underlying commands manually.
