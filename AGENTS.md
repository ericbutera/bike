# Bike monorepo

## Overview

`docs/specs/` is the source of truth for system behavior. `bike-rs` implements
the backend's business rules and data handling. `bike-ui` is the shared Next.js
and React Query frontend.
`bike-rs` owns Strava callbacks, OAuth credentials, and provider workflows.
`integrations::strava` owns provider operations and request acceptance;
`bike-core` owns stored models, retained-file parsing, and shared domain rules.
The Rust API owns map image access and caching; the separate Go
map worker owns Chromium snapshots over gRPC.

Use each project's `mise.toml` for language and package-manager versions and
tasks. Root tasks delegate to their owning projects. Preserve unrelated work
in other checkouts.

Track active Bike work in `docs/TODO.md`. Work one bounded task at a time and
deliver its result for review on a feature branch. Preserve historical
operational evidence unless a task explicitly retires it.

Host development execution is prohibited: this project generated almost 500 GB
of scattered host output in one day. Speed, an installed toolchain, and an old
approval rule do not justify an exception. This applies to mise itself, Cargo,
rustup, Next.js, package managers, editor builds, hooks, and one-off scripts.
Run builds, dependency installation, formatting, linting, type checks, tests,
coverage, code generation, and other cache-writing work inside a Docker
container. Run the existing mise commands unchanged inside that container;
do not wrap individual tasks or prek entries in Docker-dispatch scripts.
Git and GitHub may use host authentication. Enter the checks container
with Docker directly; run mise only inside the container.
Git and GitHub commands may also run inside the container using the existing
host `gh` login's temporary token, as described in the development guide.
Never persist authentication tokens in source, images, or build caches.
Follow the [shared build storage contract](docs/development.md#shared-build-storage):
all Bike caches, artifacts, scratch, and temporary output share one 200 GiB
quota. Keep databases, uploads, and other user data outside this volume.
Do not link or mount Docker caches into host `.cache`, `.scratch`, or
`.artifacts` directories. Those host paths must be safe to purge independently
of Docker, without removing configuration or breaking local development.
Remove task scratch when work finishes or its PR merges; preserve only
requested deliverables and output belonging to active work.
Run Playwright/Chromium, visual previews, screenshots, and one-off browser checks
in the pinned browser container. Keep browser warnings and errors fatal. Retiring
cross-implementation parity does not retire the current UI's light/dark appearance
tests or reviewed screenshot baselines.

## specs

specs are the source of truth- not code. code is disposable now. do not release features without updating specs. if there isn't a spec, make one. prefer using bullet lists over blobs of text. use mermaid diagrams where appropriate.

always update specs before writing code. make sure features aren't described multiple times throughout specs. make sure specs don't drift from implementation.

## git

Keep the primary editor checkout on host `main`. Agent clones and worktrees live
under `worktrees/` on the shared Docker disk and count toward its 200 GiB quota.
Create and use them inside Docker as described in the development guide; never
create host `.worktrees`, extra host clones, or source archives. Reuse an assigned
Docker checkout and shared caches. Publish completed work before removing its
checkout; preserve active uncommitted source. Git/GitHub may use host authentication,
but that exception does not authorize host worktrees or host hook execution.

Create PRs for work. Keep PR descriptions & remote code updated for review when you've completed work.

Requests to create, update, fix, or finish a PR authorize the commits,
amendments, pushes, and PR updates needed to complete that work. Do not ask
for separate confirmation to commit or push. Updating an existing PR also
authorizes rewriting its feature branch with `--force-with-lease` against a
freshly verified remote head. Honor explicit instructions such as "do not
commit", "commit locally", or "do not push". Merge or deploy only when the
user authorizes those actions.

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

Keep a clean, linear history. Do not make multiple commits for the same thing, use amend & force with lease instead.

Treat `.artifacts` as temporary generated output, not a code archive. Do not
retain source snapshots, Git bundles, deployment clones, or copied worktrees
unless the user explicitly requests an archive. Keep domain knowledge in the
owning specs and runbooks; Git holds source history. At completion, remove task
scratch output and leftover worktree directories, including directories no
longer registered with Git. Preserve explicitly requested deliverables such
as published coverage reports and other active tasks' output.

Keep implementation, corrections, and tests in one coherent feature commit;
amend locally during review. Separate specifications when requested. Prepare
history consolidation and production image ancestry implications locally;
update the remote PR branch under the standing authorization above.

Do not start commits with the word add (eg: `feat: added architecture diagram`), instead use `feat: architecture diagram`.

## code standards

- Keep controllers thin: adapt HTTP input/output and call a service. Services compose application workflows and delegate meaningful steps to named helpers.  Put database queries on their owning entity/model modules. Prefer mature, type-safe frameworks and ORMs when they reduce duplicated infrastructure.  Historical migrations are append-only; preserve their order and contents.
- Notices and warnings are hard errors in compiler, linter, test,
  browser-console, and hook output. A zero exit code alone is insufficient.
- Run the affected project's linters, formatter, type checks, relevant build,
  and tests before declaring work complete. Fix causes without suppressing
  diagnostics, weakening rules, or raising thresholds to pass.
- Use Clippy and rustfmt for Rust; golangci-lint, go vet, and Go formatting
  for Go; ESLint, Prettier, and applicable type checks for JavaScript/TypeScript.
- Rust uses a 120-column maximum in `bike-rs/.rustfmt.toml`, with rustfmt's
  default small-expression heuristics. Let the formatter decide wrapping.
- Search for and reuse existing components, hooks, services, and supported
  framework/library features before repeating behavior on individual pages.
- Honor cyclomatic/cognitive complexity, function-size, and argument-count
  limits. Split cohesive steps; avoid vague helpers and parameter bags.
- Use at most two comparisons or predicate calls in a boolean condition.
  Express ownership and identity policy through named model predicates.
- Use third party SDKs instead of writing custom clients
- Verify changed behavior with focused happy-path coverage
- Bugs must have a unit test added before fixing to prevent future regressions
- Use conventional `#[cfg(test)] mod tests` modules, `tests.rs`, or descriptive
  files inside `tests/`. Do not introduce `_unit.rs` or `_tests.rs` filenames.
- PR descriptions explain behavior and intenent. Execution details such as Docker or a local environment do not belong in the description.
- Backend unit tests use PostgreSQL SeaORM mocks and shared fixtures. Do not
  introduce SQLite databases, schema substitutes, timestamp massaging, or
  compatibility logic to maintain a second persistence implementation for tests.
- Keep prek commands, filters, tool/version pins, and required hook stages
  aligned with mise tasks and CI. Update affected enforcement in the same
  change as quality tooling; validate and run hooks without bypassing them.
- Keep provider calls behind the generated SDK wrapper and use fixtures/fakes for ordinary workflow checks. Native HTTP integration tests live in `bike-rs/api/tests`; browser tests belong to `bike-ui/tests/e2e`. Reserve Playwright for focused activity, segment, and race-viewer flows; use owning unit, functional, or SQL suites for workflow rules.

These repository instructions and the
[model and fixture specification](docs/specs/model-boundaries.md) define the
team's shared standards; personal configuration is not required. Run
`mise run hooks:install` after cloning to install local hooks.

## Simple call sites, model/domain ownership, and fixtures

- Keep production call sites as named operations: extract inputs, call the
  owning service/model method, and adapt the result. Push query details,
  ownership predicates, state transitions, and domain decisions into the
  owning model/domain layer.
- Build and execute SeaORM queries inside the owning entity/model module.
  Controllers, services, workers, and lifecycle orchestration must not chain
  `Entity::find`, `filter`, `join`, `select`, `order_by`, `count`, `update_many`,
  or `delete_many`. This includes one-off queries; reuse is not a prerequisite
  for putting a query on its owner.
- Construct and persist SeaORM ActiveModels inside their owning entity/model
  module. Callers pass domain inputs to named operations; they must not build
  mutation patches, convert stored models into ActiveModels, or receive
  ActiveModels to finish a write.
- Share cohesive field mappings on the owning model/type. Use typed inputs,
  consuming mapping methods, and struct-update syntax so operations specify only
  meaningful differences. Preserve identity and omitted-versus-cleared fields;
  creation defaults must not reset unrelated fields during updates.
- Insert constructors set only business inputs and meaningful overrides. Omit
  generated IDs, null placeholders, and values already supplied by model/database
  defaults. Keep defaults in entity metadata or insert behavior; leave omitted
  update fields unset and retain explicit clears/resets when they change stored state.
- Search for an existing owning method first. Extend it or add a cohesive,
  purpose-named operation when missing. Keep IDs and ownership scope explicit.
  Return domain values/results, not a query builder for callers to finish.
- Keep transactions, locks, compare-and-swap fences, pagination, and affected-row
  checks intact when moving queries. Complex domain methods must still honor
  the existing function-size, argument-count, and complexity limits.
- Use fixture factories with shared test helpers across languages. Fixtures
  provide valid, reusable defaults; tests override only fields relevant to their
  scenario. Keep routine test setup shared so each test shows only its meaningful
  inputs, actions, and assertions.
- Before constructing a test model, search `bike-core/src/test_support` and the
  owning feature's fixtures. Reuse or extend the canonical fixture instead of
  copying a complete `Model` or `ActiveModel` into another test module.
- Shared cross-crate Rust fixtures live in `bike-core::test_support`, behind
  `cfg(test)` or the existing `test-fixtures` feature. Enable that feature only
  in consuming crates' dev-dependencies. Override scenario fields at the test
  call site with struct updates; keep fixture time deterministic unless the
  test explicitly exercises the current clock.
- Use fixtures with an unset primary key for inserts that need generated IDs.
  Partial ActiveModels that update specific stored fields are intentional
  patches, not duplicate seed constructors. Tests may query stored state
  directly to verify results; keep production policy in the owning model.
- Use `bike_core::test_support::insert` and `update` for successful fixture
  writes throughout Rust unit tests (`crate::test_support` inside core).
  Keep scenario overrides visible with struct updates. Update patches use
  `..Default::default()` so omitted fields remain untouched. Keep direct bulk
  writes and expected-error calls when they are the behavior under test.

## mise package manager

This project uses mise for all language and package-manager tooling.

Before running Node, Rust, Go, Python, pnpm, npm, cargo, task, or similar commands:

- check `mise.toml`
- enter the development checks container described in [local development](docs/development.md)
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

Keep tool versions and application build-image pins in mise only. Dockerfiles
declare build arguments without version defaults; Compose requires the exported
mise values. Do not copy a pin into a Dockerfile or Compose fallback during an
upgrade. Regenerate pnpm's required manifest/lock metadata with
`mise run pins:sync`; `mise run pins:check`, UI checks, and prek verify freshness.

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

Keep private infrastructure repository names, URLs, and local paths out of
public files. CI receives repository locations through generically named secrets.

## other rules

- Clean up completed Docker Git worktrees after their PRs merge.
- Do not introduce new scripting languages without explicit approval. Only shell scripts are allowed.
- Create shell scripts instead of multi-line commands.
- Use mise as the task runner.
