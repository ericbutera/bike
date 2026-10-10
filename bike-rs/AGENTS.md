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

Run only focused tests and lints for the code touched locally, in Docker. For
worker changes, use `mise run test:workers`; select the owning crate and test
filter for other changes. The primary full test/lint gate is CI/CD through
`ci:rust`, including workspace unit coverage, integration tests, doctests, Clippy,
formatting, and contract checks. Keep tests under the owning crate's source or
`tests/` path so these tasks discover them. Keep required prek checks active.

Do not repeat full workspace tests, coverage, or builds locally, and do not
repeatedly poll CI completion. Update the draft PR and report CI status separately
from focused local results.
