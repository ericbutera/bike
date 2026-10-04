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

## Verification

Add or update happy-path test coverage for every feature change.

Do not start dev servers or Next.js services without task authorization. Use
the existing Playwright harness against the local Rust stack for focused
browser verification, with fakes at external-service seams. Missing in-app
Browser controls alone do not require a manual user handoff.

Prefer targeted local checks for the code touched. The project CI is configured
to fail on tests, lint, and formatting, so do not spend quota repeatedly polling
for CI completion. Before starting any long-running verification, CI watch, or
status polling loop, ask the user whether to proceed.
