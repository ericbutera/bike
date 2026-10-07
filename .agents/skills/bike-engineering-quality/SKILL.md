---
name: bike-engineering-quality
description: Apply Bike's shared engineering quality workflow during code changes, review, commits, and lint, prek, or CI maintenance.
---

# Bike engineering quality

Use the repository's root `AGENTS.md` as the source of shared engineering
standards and read applicable component instructions. This workflow uses
repository files and tooling; no personal Codex configuration is required.

## Establish the checks

Read the applicable `AGENTS.md`, including its Git rules, `mise.toml`,
package/workspace manifests, linter configuration, and existing prek/CI gates.
Prefer established mise tasks over recreating commands. Keep task orchestration
in mise config, with direct native tool commands and calls to owning tasks.
Multi-step operational workflows may use Bash scripts invoked by mise. Keep
their tool/image pins in mise and check them with the owning ShellCheck task
in local lint, prek, and CI.
Do not create custom wrappers, config parsers, or validator frameworks for
operations mise and the native tools already support. Run tools through mise;
install missing configured tools with `mise install`.

CI/CD must invoke the same owning named mise tasks used locally. Keep language
tool versions in mise config and remove duplicated commands or version pins
from workflow YAML. Source mise from its official pinned image on runners and
share its executable through ignored CI artifacts. Do not commit mise binaries
or installers; language checks belong in owning mise tasks.

Identify the required formatter, linter, type checks, build, and tests for the
affected area. Inspect configured complexity limits before writing or
refactoring substantial functions. When changing quality tooling, enforce
warnings and supported complexity rules as failures in local and CI commands.
Examples include ESLint `--max-warnings=0` and Clippy `-- -D warnings`;
Clippy's `all` group alone does not enable every complexity lint.

Use the owning language's quality tools through its configured tasks:

| Language              | Required checks where applicable                                               |
| --------------------- | ------------------------------------------------------------------------------ |
| Rust                  | rustfmt and Clippy with warnings denied, using configured targets and features |
| Go                    | golangci-lint, go vet, and the configured Go formatting check                  |
| JavaScript/TypeScript | ESLint with zero warnings, Prettier checks, and configured type checks         |

Compilation and tests do not replace linting. Go vet alone does not replace
golangci-lint, and a focused subset of lint rules does not establish that the
full configured gate passed. Identify and fix missing enforcement in affected
quality tooling; report any unresolved gap explicitly.

## Maintain prek and CI enforcement

When quality tooling changes, update affected mise/package tasks, prek hooks,
and CI call sites together. Hooks should invoke the same strict owning tasks
as CI. Check that file filters include relevant source and quality configuration
files. Install required hook stages through the repository's hook-install task.

When maintaining hooks, check tool pins and remote-hook revisions for stale
versions. Update compatible versions through the project's existing update
mechanism and verify the resulting gates. Keep versions consistent with mise
and project manifests. Preserve the existing configuration format; avoid a
second hook configuration or duplicate check implementation.

When establishing or upgrading commit validation, enforce the repository's
Conventional Commit format and allowed types at the `commit-msg` stage.
Do not skip hooks or use `--no-verify` to conceal a failing quality gate.

## Design for reuse and clarity

Use the owning project's ORM and query builder for ordinary database access.
In Bike Rust, use SeaORM entities, ActiveModels, typed projections, filters,
joins, aggregates, pagination, bulk inserts, update/delete builders, locking,
and supported conflict APIs. Do not write raw SQL for operations these APIs
express directly, or merely because SQL is quicker to type. Put query
construction on the owning entity/model; services compose workflows and
controllers adapt HTTP. Reserve raw SQL for complex operations that cannot be
expressed clearly with the supported builder, database-specific primitives,
and migration/fixture DDL. Document the concrete reason at each application
escape hatch and bind dynamic values. Preserve transaction, ownership,
ordering, conflict, lease, and affected-row behavior when converting queries;
verify those invariants against the owning database tests.

Use `rg` to find existing components, hooks, model methods, services, and tests
before adding similar behavior. Share repeated UI behavior through an owning
component or hook. Prefer existing, mature framework/library features to
custom infrastructure. Extract cohesive abstractions with specific interfaces.

Keep orchestration separate from meaningful stage behavior. Reduce excessive
branching and nesting through named steps and clear control flow. Honor the
project's cyclomatic/cognitive complexity, function-length, and argument-count
thresholds. If no complexity rule is configured, review complexity explicitly
and distinguish that review from automated enforcement. Avoid vague parameter
bags and helpers that merely relocate a large function.

Fix causes instead of adding lint disables, diagnostic filters, empty catch
blocks, or weaker configuration. Preserve unrelated work and keep fixes within
the requested scope; identify a separate blocker if a clean gate needs work
outside that scope.

## Keep product specifications current

For changes to product behavior, business rules, or background processing,
read the [spec index](../../../bike-rs/docs/specs/README.md) and the owning
domain spec before implementation. Update the canonical behavior in the same
change; an appended investigation log does not replace obsolete rules. Reuse
the owning spec instead of creating an overlapping one. Keep task status in
the backlog and distinguish proposed behavior, local implementation, and
verified production behavior. Include the updated spec links in the final
report. Reversible implementation details without a behavior change do not
need a new product specification.

## Verify before completion

Run the affected area's established formatter, linter, type checks, relevant
build, and focused tests. Use meaningful happy-path coverage for changed
behavior and a regression check for an observed bug. Run relevant existing
prek hooks when source or hook configuration changes. Documentation-only
changes need applicable document/config validation, not application suites.

Respect the requested test level. Unit coverage uses pure functions, mocked
boundaries, or in-memory databases with fixtures. Do not introduce database
servers or CI services to satisfy a unit-test request. Keep server-specific
database verification separate and opt-in unless the user explicitly asks for
that integration coverage in CI.

Inspect diagnostics as well as exit codes. Compiler, dependency, test-runner,
React/browser-console, linter, and hook warnings/notices are failures even if
the command exits successfully. Resolve them and rerun the affected gate.
Existing warnings also prevent claiming that gate passed cleanly. If a
dependency, environment, or unrelated defect prevents completion, state the
exact failing check and cause; do not label it passed or silently skip it.

Review the final diff for duplicated behavior, complexity, suppression, and
unrelated edits; check whitespace with `git diff --check`. Before a commit,
apply the repository's Git instructions: use an allowed Conventional Commit
type and a concise subject describing the result without starting with "add",
"added", or "adding". Keep continuing work in the same logical commit when
amending is authorized; preserve unrelated staged changes and use
`--force-with-lease` for an authorized rewritten-history push.

Do not push until the user explicitly signs off the completed feature and
authorizes publishing the reviewed commits. Passing checks, continuation
requests, and a previous feature's release approval do not satisfy this gate.
Combine implementation, corrections, and tests into one coherent feature
commit, with separate specification commits when requested. Keep work local
and amend during review. Prepare any already-published history consolidation
and its production ancestry implications before seeking remote rewrite approval;
publish rewritten history only with `--force-with-lease` after sign-off.

Report what changed and which checks actually passed. Keep local, observed CI,
and deployed verification distinct. For reviews, report violations and missing
evidence without editing files unless authorized.
