# Bike monorepo

## Overview

`bike-rs` is the Bike backend and source of truth for business behavior and
data handling. `bike-ui` is the shared Next.js and React Query frontend.
`strava-gateway` owns the provider callback, inbox, credentials, and delivery
to Bike Rust. The map renderer is a separate shared service.

Use each project's `mise.toml` for language and package-manager versions and
tasks. Root tasks delegate to their owning projects. Preserve unrelated work
in other checkouts.

Keep controllers thin: adapt HTTP input/output and call a service. Services
compose application workflows and delegate meaningful steps to named helpers.
Put database queries on their owning entity/model modules. Prefer mature,
type-safe frameworks and ORMs when they reduce duplicated infrastructure.
Historical migrations are append-only; preserve their order and contents.

Track active Bike work in `docs/TODO.md`. Work one bounded task at a time and
commit its result with a conventional commit message. Preserve historical
operational evidence unless a task explicitly retires it.

Use focused checks through the relevant mise task. Keep provider calls behind
the gateway seam and use fixtures/fakes for ordinary workflow checks. Native
HTTP integration tests live in `bike-rs/api/tests`; browser tests belong to
`bike-ui/tests/e2e`. Reserve Playwright for focused activity, segment, and
race-viewer flows; use owning unit, functional, or SQL suites for workflow rules.

## git

Always use conventional commit format.

types:

- feat
- fix
- perf
- style
- ops
- docs
- test
- ci

Keep a clean history. Do not make multiple commits for the same thing, use amend & force with lease instead.

Do not push to a remote until the user explicitly signs off the completed
feature and authorizes publishing its reviewed commits. Keep implementation,
corrections, and tests in one coherent feature commit; amend locally while
review is ongoing. Separate specifications when requested. Passing checks,
requests to continue, and approval of a previous feature are not sign-off for
new work. Prepare history consolidation locally before requesting publication;
use `--force-with-lease` only after an approved remote rewrite. Review any
production image ancestry implications before publishing rewritten history.

Do not start commits with the word add (eg: `feat: added architecture diagram`), instead use `feat: architecture diagram`.

## code standards

- Notices and warnings are hard errors in compiler, linter, test,
  browser-console, and hook output. A zero exit code alone is insufficient.
- Run the affected project's linters, formatter, type checks, relevant build,
  and tests before declaring work complete. Fix causes without suppressing
  diagnostics, weakening rules, or raising thresholds to pass.
- Use Clippy and rustfmt for Rust; golangci-lint, go vet, and Go formatting
  for Go; ESLint, Prettier, and applicable type checks for JavaScript/TypeScript.
- Search for and reuse existing components, hooks, services, and supported
  framework/library features before repeating behavior on individual pages.
- Honor cyclomatic/cognitive complexity, function-size, and argument-count
  limits. Split cohesive steps; avoid vague helpers and parameter bags.
- Verify changed behavior with focused happy-path coverage and regression
  checks for observed bugs. Distinguish local, CI, and deployed evidence.
- Keep prek commands, filters, tool/version pins, and required hook stages
  aligned with mise tasks and CI. Update affected enforcement in the same
  change as quality tooling; validate and run hooks without bypassing them.

For code changes, review, commits, and quality-tooling maintenance, read
[the shared engineering quality skill](.agents/skills/bike-engineering-quality/SKILL.md)
once per session and apply its workflow. This repository's instructions and
skill define the team's shared standards; personal configuration is not
required. Run `mise run hooks:install` after cloning to install local hooks.

## mise package manager

This project uses mise for all language and package-manager tooling.

Before running Node, Rust, Go, Python, pnpm, npm, cargo, task, or similar commands:

- check `mise.toml`
- run tools through `mise exec -- <command>`
- if tools are missing, run `mise install`

Prefer an existing named `mise run <task>` over a raw tool command. Keep
`lint` as direct calls to the owning `lint:rs`, `lint:go`, `lint:ui`,
`lint:maps`, and `lint:sh` tasks. Use native tool commands for simple tasks.
Multi-step operational workflows may live in Bash scripts called by mise;
mise owns their tool/image pins, and ShellCheck runs locally, in prek, and CI.
Do not create custom configuration parsers or validator frameworks. CI/CD must
call the same owning tasks used locally; keep tool versions in mise config and
avoid duplicating commands or pins in workflow YAML. The official pinned mise
image supplies the runner executable through ignored CI artifacts; do not commit
mise or its installer. Language checks belong in the owning tasks.

Examples:

- `mise exec -- pnpm exec tsc --noEmit`
- `mise exec -- pnpm test`
- `mise exec -- cargo test`

## specs

Keep shared documentation in the root `docs/` directory and product
specifications in `docs/specs/`. Link to these documents from component READMEs
instead of maintaining component documentation copies. Business rules and
decisions belong in the owning specification. Generated HTTP contracts live
only in `contracts/openapi/`; Rust generates them directly there.
