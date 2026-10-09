# Agent Instructions

## Tooling

This project uses `mise` as the task runner and tool version manager.

Before running Rust, Node, pnpm, npm, cargo, test, lint, build, or project
maintenance commands, check `mise.toml` and prefer the existing mise tasks.

Examples:

- `mise tasks`
- `mise run test`
- `mise run lint`
- `mise run openapi:typescript:check`

If a task exists for the workflow, use that task instead of spelling out the
underlying commands manually.

CI/CD must reuse these owning mise tasks. Follow the root Git authorization
rules: permission to commit includes publication, integration, and cleanup.
Keep each complete feature in one coherent commit.

## Verification

Add or update happy-path test coverage for every feature change.

Do not start dev servers or Next.js services without task authorization. Use
the existing Playwright harness against the local Rust stack for focused
browser verification, with fakes at external-service seams. Missing in-app
Browser controls alone do not require a manual user handoff.

Prefer targeted local checks for the code touched. The project CI is configured
to fail on tests, lint, and formatting, so do not spend quota repeatedly polling
for CI completion. Run the verification needed for the authorized task. Ask
before starting expensive verification outside that scope; do not ask again
when the user has already authorized the checks or deployment verification.
