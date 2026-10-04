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

## Git

Always use conventional commit format:

- `feat`
- `fix`
- `perf`
- `style`
- `ops`
- `docs`
- `test`
