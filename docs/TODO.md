# Bike backlog

This is the active checklist. [Product specifications](specs/README.md)
define behavior. Completed migration and deployment records remain in Git.
Keep private activity data out of committed reports and fixtures.

## Active work

**WORK01 — Proposed:** Deliver received-to-available pipeline visibility using the
existing worker. Follow the [visibility plan](plans/pipeline-visibility.md):
connect all processors and gateway work with durable run/attempt lineage,
persisted original-receipt timestamps and request/trace IDs; show full DAG/timing
and related tasks from an admin activity; measure receipt-to-current/available,
per-type p50/p90, retries, waits, stalls and required-output readiness; expose
work amplification, load/capacity and redundant heatmap/fitness/segment rebuilds;
update provisioned Grafana dashboards with distributions, worker health and
individual anomalous runs; refactor meaningful queue boundaries; verify live
correlation, diagnostic links, panel queries and alerts.
The [current flow diagrams](plans/worker-current-flows.md) document existing
triggers and handoffs. Implementation has not started.

Deployment simplification is implemented locally: `mise run ci:deploy` clones
IaC once, compiles the consolidated Bike Pulumi program once, and applies all
four image pins in one update. All workload definitions now use the `bike`
namespace and Bike image repositories. Publishing, the artifact data copy,
and the production namespace/state cutover remain pending; follow the
[deployment guide](deployment.md#image-promotion) before the first release.

| ID     | State                | Action                                                                          | Completion criteria                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------ | -------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| DATA04 | Pending verification | Bound archive working sets and format preference.                               | Verify file-backed descriptors, one bounded expanded member, FIT/TCX/GPX preference, retained partial progress/errors, and peak memory on an existing archive fixture.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| DATA05 | Pending verification | Match the current import and refresh affected analytics.                        | Include shared segments in bounded pages; refresh displaced activities and old/new segment summaries; preserve per-activity effort numbering through regeneration and sport changes. Verify workflow, SQL, and working set.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| DATA06 | Pending verification | Separate training inputs from report summaries and bound backfills.             | Read complete ordered samples for one owned ride after matching; backfill scalar IDs without retaining whole-history routes. Verify owning behavior, SQL, and memory.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| DATA18 | Awaiting originals   | Recover authentic sources for remaining generated Strava TCX records.           | [Recovery contract](specs/activity-ingestion.md#generated-strava-tcx-retirement-and-source-backfill): obtain missing originals from an updated archive or explicitly retire those sources; replay and verify eligible outdoor controls and zero virtual contributions. Preserve summaries/GPS while sources remain unavailable.                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| DATA19 | Proposed             | Separate normalized GPS/chart/lap data from activity summaries.                 | [Storage proposal](specs/activity-ingestion.md#proposed-separation-of-activity-detail-from-summaries): owned one-to-one activity details, lightweight recording/admission decisions, bounded verified backfill and transactional replay/deletion, explicit detail reads, and measured bytes/latency/memory before considering per-point or chunked storage. No schema migration implemented.                                                                                                                                                                                                                                                                                                                                                                                          |
| DATA12 | Implemented          | Follow imports and replay from recorded stages.                                 | Owner history includes every source and failed imports without an activity. Durable attempts preserve summaries and errors; replay reuses valid prerequisites or explains a fallback. Recovery respects live import/archive/bulk task heartbeats. Rust and UI unit checks passed, including focused history/replay coverage. CI owns release verification; an actual production recovery operation remains separate. Includes REC05.                                                                                                                                                                                                                                                                                                                                                  |
| ACT02  | Pending verification | Confirm Garmin FIT sub-sport classification and targeted archive reprocessing.  | Record importer behavior and production recovery for an existing relevant FIT recording.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ACT05  | Pending              | Retain non-cycling archive/upload inputs without ride processing.               | [Retention contract](specs/activity-ingestion.md#non-cycling-retention-proposal): owned originals and minimal summaries, explicit deferred outcome, zero full GPS/detail decodes or downstream ride jobs, idempotent future promotion, and measured mixed-sport import costs. Reuse import/artifact storage. Existing gateway delivers no non-cycling summary; gateway changes are outside scope.                                                                                                                                                                                                                                                                                                                                                                                     |
| MAPS12 | Pending              | Require explicit heatmap admission and withhold unknown recordings.             | [Admission proposal](specs/activity-ingestion.md#heatmap-admission-proposal): agree supported outdoor evidence rules and their trust limits; persist a versioned decision separate from claimed environment; enforce all ingestion, replay, preparation, publication, and read boundaries; positive outdoor and unknown/virtual controls; two-user isolation; migrations/backfill and live verification before second-user imports. This stricter policy is not implemented by v5.                                                                                                                                                                                                                                                                                                    |
| MAPS13 | Pending              | Define a separate global cycling heatmap after personal admission is verified.  | [Scope](specs/heatmaps.md#cycling-admission-proposal-2026-10-06): explicit participation, stricter contribution admission, separate aggregate queries/revisions, removal on opt-out/deletion/reclassification, no automatic global approval from personal overrides, and two-owner correctness tests. No global endpoint is implemented.                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| MAPS14 | Implemented locally  | Refine personal heatmap framing and controls.                                   | [Map controls](specs/heatmaps.md#page-layout-and-map-lifecycle): automatic location at zoom 13 with route-area fallback; saved/user-selected views take precedence; loading resize events cannot save the temporary world view, and old `0,0,2` URLs recover; GPS below zoom-out; Region fits the current state/province, Full fits filtered route bounds; help follows Filters and replaces the top-left card. 193 UI tests, five fixture browser checks, build, and strict prek gates pass. Production verification remains pending.                                                                                                                                                                                                                                                |
| TEST10 | Implemented locally  | Separate synthetic availability monitoring from browser e2e.                    | Separate k6 availability and Playwright images build and pass fixture checks. Production publication and runtime verification remain separate from local implementation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| TEST11 | Deployed             | Run containerized Playwright locally and in CI against disposable environments. | [Implementation and acceptance](E2E-TODO.md#production-verification): Bike PR #8 and companion IaC PR #6 are merged. Main pipeline 221 passed all 74 browser tests in 3m55s, completed the E2E step in 5m09s, and deployed the tested digests through Bike update 200. All six production workloads are ready with zero restarts; both migrations, public availability, and the internal read-only synthetic database query passed. Local full-suite execution passed in 3.4 minutes. Persistent BuildKit built all seven warm images in 20 seconds; ORM seeds and native snapshots reset data without restarting services or replacing schema/pools. Explicit fault-injection, registry-boundary, and runner-cancellation acceptance remains open. Worker E2E is future opt-in work. |
| TEST12 | Implemented locally  | Generate Rust/Next.js reports and enforce coverage for changed lines in CI.     | [Coverage guide](development.md#ci-coverage-policy-and-viewing-reports): 80% changed-line coverage per project; untouched existing source has no minimum, Rust migrations are excluded, and overall percentages are informational. Free [GitHub Pages reports](https://ericbutera.github.io/bike/coverage/) provide HTML, diff reports, downloads, and ten-revision history. [Happy-path checklist](coverage-checklist.md): 25 new tests cover 10 Rust files and 11 Next.js paths; 49 Rust and 43 Next.js paths remain at 0%. Integrated local unit reports: Rust 57.55% lines (294 passed, three ignored), Next.js 58.70% lines and 45.25% branches (214 passed). Policy fixtures, owning language checks, and workflow validation passed.                                           |

## System upgrade checklist

Version audit checked on **2026-10-06**. Checked items in the first group are
implemented and verified locally; production deployment remains separate.
Unchecked items are planned work. Targets are released stable versions
verified from upstream policies and registries. Refresh their patch versions
before implementation. Prefer the latest supported LTS line where one exists;
Rust, Go, npm, pnpm, and most libraries do not have a Node-style LTS channel.

Scope: root/component mise configs, application manifests and lockfiles, all
tracked Dockerfiles, Compose, Woodpecker, code generators, the vendored Rust
patch. Infrastructure definitions were
checked only for the database baseline; running production versions, host
software, image digests, and container OS package vulnerabilities still require
deployment inspection. This checklist does not certify the absence of unused
code or documentation.

### Security and unsupported software first

- [x] **UPG01 — Patch Next.js and its production dependencies.**
      Next/ESLint config **16.4.0**, React/React DOM **19.3.0**, and refreshed
      production locks replace the affected versions. `sharp` resolves **0.35.5**,
      PostCSS **8.5.23/8.5.28**, and `nanoid` **3.3.19**. This removes the
      [AVIF advisory](https://github.com/vercel/next.js/security/advisories/GHSA-2xp9-vwfh-vxw4)
      affecting the previous Next 16.1.6 pin. UI unit/contract checks and production
      build/image pass; the image serves login, maps, and activity SSR documents.
      OAuth actions use ordinary links so provider redirects perform a full
      document navigation; relative and external API bases have unit coverage.
      Browser checks cover fake-provider sign-in, session reload/logout, and
      non-admin/forbidden reads; the optional unauthorized-route matrix was not selected.
      Sources: [Next support policy](https://nextjs.org/support-policy),
      [Next registry](https://registry.npmjs.org/next/latest),
      [React registry](https://registry.npmjs.org/react/latest).
- [x] **UPG02 — Unify supported MapLibre versions.** Both UI and renderer pin
      **6.13.0**, replacing UI 5.24.0 and renderer 6.11.2. The UI now uses the
      supported namespace exports and typed paint keys. Map/heatmap/segment/race
      unit fixtures pass; the renderer image passes authenticated HTTP/gRPC PNG,
      theme, thumbnail, scale, and cache checks. GPS admission/filtering is preserved.
      This removes the previous UI version's
      [sanitizer advisory](https://github.com/maplibre/maplibre-gl-js/security/advisories/GHSA-jrc7-96c5-q579).
      Source:
      [MapLibre registry](https://registry.npmjs.org/maplibre-gl/latest).
- [x] **UPG03 — Replace the unsupported Go toolchain.** Mise, gateway Docker,
      and the module minimum now select **1.27.1**. `GOTOOLCHAIN=local` in owning
      tasks and the builder prevents compiler substitution. Gateway formatting,
      vet, golangci-lint, fixture/unit tests, all command builds, protobuf freshness,
      and a linux/amd64 gateway image pass. Go supports 1.26 and 1.27 at this snapshot.
      Source:
      [Go releases and support policy](https://go.dev/doc/devel/release).
- [x] **UPG04 — Move UI ESLint off its EOL major.** UI and renderer use
      **10.12.0**, with strict rules and zero-warning enforcement. ESLint 9 ended
      maintenance on 2026-08-06. Next 16.4 supplies a rule-context adapter, but
      three bundled plugins still declare older ESLint peers: exact-version pnpm
      overrides record their tested compatibility through that adapter. Regression
      fixtures accept valid code and detect React, accessibility, import, and Next
      violations; strict peer validation and full lint pass. Remove those peer
      overrides when the upstream declarations include 10. Source:
      [ESLint support policy](https://eslint.org/version-support/).
- [x] **UPG05 — Replace Jaeger 1 in local tracing.** Mise and Compose pin
      `cr.jaegertracing.io/jaegertracing/jaeger:2.22.0`, using its built-in
      all-in-one configuration. The obsolete v1 collector flag is removed.
      The optional tracing profile retains ports 16686/4317/4318. A disposable
      container accepted an OTLP JSON span and returned it through the v3 query API;
      UI availability and Compose configuration pass. No production/IaC change was made.
      Sources: [Jaeger lifecycle](https://www.jaegertracing.io/download/),
      [2.22.0 release](https://github.com/jaegertracing/jaeger/releases/tag/v2.22.0).
- [x] **UPG06 — Replace unmaintained cargo-watch.** Mise and both Rust dev
      images pin [watchexec CLI **2.8.0**](https://github.com/watchexec/watchexec).
      Compose passes the shared pin; both image commands restart the owning Cargo
      binary directly and use SIGTERM to stop it. Old pins/installations are removed.
      Both images build and pass startup, file-change restart, visible failure,
      recovery, and graceful shutdown checks using a fake Cargo command. These
      are watcher/process checks, not a new database or full import replay. Source:
      [cargo-watch maintenance statement](https://github.com/watchexec/cargo-watch#maintenance).
- [x] **UPG07 — Resolve the remaining UI audit findings.** Both native JS audits
      report **zero advisories**, including the UI development tree (previously
      3 critical, 23 high, 20 moderate, 6 low). Vitest **4.1.11**, Vite **8.3.2**,
      happy-dom **20.14.5**, Mermaid **11.17.2**, and Tailwind **4.3.3** resolve
      patched parent/transitive releases. Two exact consumer overrides use KaTeX
      **0.18.2** and replace Next's `fast-glob` with **tinyglobby 0.2.17**,
      removing the unpatched `braces` path. Root glob and Mermaid math fixtures
      exercise those interfaces; valid existing UI tests pass. Vite's native ESM
      config warning is fixed by explicit module metadata and URL-based paths.
      Owning UI and renderer checks now fail on native audit findings; UI checks
      also reject incompatible peers. RustSec, Go, and container-layer scans remain
      separate work in UPG22; zero JS advisories is not a complete system security audit.

### Toolchains, database, and images

Completed in the review branches on 2026-10-06. These checks establish local
implementation and isolated compatibility, not a production deployment.
The shared database's production patch requires the companion infrastructure
change to be reviewed, its backup image rebuilt/published, and the StatefulSet
applied.
The observed production/local servers remain 17.7/17.9 until that rollout.

- [x] **UPG08 — Use current stable Rust consistently.** Application mise and
      official build/development images use **1.99.0**, with its Clippy/rustfmt.
      New slice lints use native array chunks; **async-trait 0.1.92** fixes the
      older macro's generated warning. The unused spatial experiment and its
      separate compiler/image controls have been removed.
      [Rust release](https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/).
- [x] **UPG09 — Align and patch PostgreSQL before a major migration.** Bike,
      optional gateway fixtures, and the companion production/backup definitions
      use the same explicit **17.11-trixie** multi-platform digest. Separate
      server checks remain opt-in; no database CI service was introduced.
      Live patch rollout remains subject to review as described above.
      [PostgreSQL support policy](https://www.postgresql.org/support/versioning/).
- [x] **UPG10 — Rehearse PostgreSQL 18 before changing persistent instances.**
      The owning `db:rehearse` task restores private local/production Bike dumps
      on **17.11** and **18.6** in fresh volumes, checks all public table counts,
      exercises existing heatmap/gateway PostgreSQL cases, and restarts the
      retained 17 instance for recovery. It uses random loopback ports and
      removes only its own resources. [Deployment documentation](deployment.md#postgresql-patching-and-major-upgrade-rehearsal)
      records the changed 18 volume layout, observed `plpgsql` extension scope,
      backups, write freeze, cutover and recovery limits. No live major upgrade
      was performed.
      [Official image notes](https://github.com/docker-library/docs/blob/master/postgres/README.md).
- [x] **UPG11 — Refresh and pin base images.** Rust, Go, browser e2e, renderer's
      Node stage, runtime and CI bootstrap use **trixie**, with immutable image
      digests. UI pins **Alpine 3.24**. Playwright **1.63.0/noble** retains its
      matching browser/library identity. Clone **2.10.1**, Buildx **0.38.0**, k6
      **2.3.0**, and database inputs are pinned. Application SHA tags remain the
      release identities. Base inputs use immutable references.
      [Debian lifecycle](https://www.debian.org/releases/).
- [x] **UPG12 — Refresh quality-tool pins.** Prettier **3.9.9** and prek
      **0.5.5** replace 3.7.4/0.4.12. Seven UI files, including the generated API
      types, were reformatted; native config validation and reinstalled hooks
      use the same owning mise gates. Root toolchain edits trigger Rust hooks.
      [prek release](https://github.com/j178/prek/releases/tag/v0.5.5).
- [x] **UPG13 — Align protobuf generators with runtimes.** Go protobuf
      generator **1.36.12** matches its runtime; gRPC generator **1.6.2** has
      freshly generated bindings. Both languages use root **protoc 36.2**;
      Rust's separate vendored compiler was removed. Rust Docker builds copy
      the selected compiler/includes from an official mise image stage. Native
      freshness and gateway/receiver tests cover the updated bindings.
- [x] **UPG14 — Make mise pins reach every consumer.** Native image tasks and
      Compose pass the selected image/tool pins. Local and CI release tasks use
      the same Bake targets and mise environment. BuildKit retains package and
      compiler caches across jobs; API/worker share one compilation stage.
      Official mise **2026.10.3/debian**
      supplies CI and protobuf stages; its installer is not committed. Cargo-chef
      dependency caching is restored with immutable upstream revision
      `449576bbc2645200936adb9dece80810c9a335f8` (PR #369), fixing Cargo 1.99's
      target-edition warnings without suppressions. Replace the temporary revision
      once the fix is released. Docker defaults and pre-bootstrap/plugin digests remain
      documented explicit synchronization points, rather than another parser.
      [Version ownership](deployment.md#build-version-ownership).

### Application library migrations

Libraries below have newer stable releases; this is not an assertion that all
older lines are EOL. Review release notes and compatibility as each task begins.
The Rust versions are from application declarations/`Cargo.lock`, not a newer
transitive copy that happens to appear elsewhere in the lockfile.

- [ ] **UPG15 — Move SeaORM off prereleases.** Workspace declares
      **2.0.0-rc.27**; the lockfile resolves **2.0.0-rc.38** for ORM and migrations.
      Upgrade both to stable **2.0.4** together. Preserve typed queries, transactions,
      ownership, conflict handling, leases, and append-only migration history;
      exercise mocks/in-memory fixtures and the separately selected PostgreSQL cases.
      Reassess the used `proc-macro-error2` **2.0.1** vendored patch after this change;
      upstream remains 2.0.1, so do not delete the patch merely because it is vendored.
      Source: [SeaORM 2.0.4](https://github.com/SeaQL/sea-orm/releases/tag/2.0.4).
- [ ] **UPG16 — Upgrade the Rust HTTP/OpenAPI stack as a coordinated change.**
      [Axum](https://crates.io/crates/axum) **0.7.9 → 0.8.9**,
      [tower-http](https://crates.io/crates/tower-http) **0.6.9 → 0.7.1**,
      [Reqwest](https://crates.io/crates/reqwest) **0.12.28 → 0.13.5**,
      [Utoipa](https://crates.io/crates/utoipa) **5.5.0 → 6.0.0**, and
      [Swagger UI](https://crates.io/crates/utoipa-swagger-ui) **8.1.0 → 10.0.1**.
      Verify route syntax/extractors, middleware, HTTP/TLS behavior, and response
      contracts. Generate OpenAPI from current Rust before checking the UI types;
      matching stale copies are insufficient. Remove the unused contracts JSON copy
      and point consumers at canonical generated artifacts where build contexts
      permit, updating the inaccurate contracts README and actual call sites.
- [ ] **UPG17 — Upgrade Rust auth, validation, and scheduling libraries.**
      [jsonwebtoken](https://crates.io/crates/jsonwebtoken) **9.3.1 → 11.1.0**,
      direct [OAuth2](https://crates.io/crates/oauth2) **4.4.2 → 5.0.0**,
      [validator](https://crates.io/crates/validator) **0.18.1 → 0.21.0**,
      [cron](https://crates.io/crates/cron) **0.12.1 → 0.17.0**,
      [Handlebars](https://crates.io/crates/handlebars) **5.1.2 → 6.4.4**, and
      [OpenFeature](https://crates.io/crates/open-feature) **0.2.7 → 0.3.0**.
      Preserve issuer/audience/expiry checks, sessions, OAuth state, validation
      responses, schedule/timezone behavior, and rendered messages. OpenID Connect
      **4.0.1** is already current; a transitive OAuth2 5 copy does not upgrade the
      direct 4.x dependency.
- [ ] **UPG18 — Upgrade archive, geometry, and crypto helpers with their fixtures.**
      [zip](https://crates.io/crates/zip) **2.4.2 → 8.6.0** (exclude 9 prereleases),
      [roxmltree](https://crates.io/crates/roxmltree) **0.20.0 → 0.21.1**,
      [PNG](https://crates.io/crates/png) **0.17.16 → 0.18.1**,
      [LRU](https://crates.io/crates/lru) **0.12.5 → 0.18.5**,
      [HMAC](https://crates.io/crates/hmac) **0.12.1 → 0.13.0**, and
      [SHA-2](https://crates.io/crates/sha2) **0.10.9 → 0.11.0**.
      Verify authentic archive selection, bounds/errors, retained recording metadata,
      heatmap geometry/cache behavior, signature compatibility, and outdoor/virtual
      regression fixtures. FIT parser **0.11.0** is current.
- [ ] **UPG19 — Refresh telemetry and gateway dependencies in compatible groups.**
      Rust OpenTelemetry **0.32.x → 0.33.0**, tracing integration **0.33.0 → 0.34.0**;
      renderer experimental packages **0.222.0 → 0.223.0** and stable SDK/resources
      **2.11.0 → 2.12.0**; gateway OTel **1.46.0 → 1.47.0**, HTTP/gRPC instrumentation
      **0.71.0 → 0.72.0**, `otelpgx` **0.12.0 → 0.12.1**, pgx **5.9.2 → 5.11.0**,
      Goose **3.27.0 → 3.28.0**, and gRPC **1.83.2 → 1.84.0**. Verify propagation,
      exporters, metrics, migrations, retries, and boundary fixtures. Go's module
      query found 96 available updates across 166 modules; review indirect modules
      after the direct upgrades rather than bulk-upgrading them blindly.
      Sources: [Rust OTel](https://crates.io/crates/opentelemetry),
      [JS OTel releases](https://github.com/open-telemetry/opentelemetry-js/releases),
      [Go OTel](https://pkg.go.dev/go.opentelemetry.io/otel),
      [pgx](https://pkg.go.dev/github.com/jackc/pgx/v5),
      [Goose](https://pkg.go.dev/github.com/pressly/goose/v3).
- [ ] **UPG20 — Modernize UI types, tests, and supporting libraries.**
      Node types **20.19.39 → 24.19.1** to match Node 24, not the registry's Node 26
      default; TypeScript **5.9.3 → 7.0.2**, Vitest **4.1.11 → 5.0.3**, jest-dom
      **6.9.1 → 7.0.1**, and Mermaid **11.17.2 → 12.1.0**. Vite React plugin
      **6.1.2**, happy-dom **20.14.5**, and Tailwind/PostCSS plugin **4.3.3**
      were updated by UPG07. Review TypeScript
      6/7 migration changes and test/plugin peers; verify meaningful unit assertions,
      generated types, rendering, diagrams, and fake-boundary browser checks.
      Remaining smaller updates: React Query **5.100.9 → 5.104.1**,
      DaisyUI **5.7.37 → 5.7.47**, Recharts
      **3.8.1 → 3.10.1**, Font Awesome core/icons **7.2.0 → 7.3.1**, React Font
      Awesome **3.3.1 → 3.5.0**, React/DOM types to **19.3.0**, Testing Library
      React **16.3.2 → 16.3.3**, user-event **14.6.1 → 14.6.7**, and toast
      **2.6.0 → 2.6.1**. Sources:
      [npm registry](https://registry.npmjs.org/),
      [TypeScript](https://registry.npmjs.org/typescript/latest),
      [Vitest](https://registry.npmjs.org/vitest/latest).
- [ ] **UPG21 — Refresh compatible Rust lockfile releases after migrations.**
      Application Tokio **1.52.2 → 1.53.2**, Serde **1.0.228 → 1.0.229**,
      async-trait **0.1.89 → 0.1.92**, Chrono **0.4.44 → 0.4.45**, lettre
      **0.11.21 → 0.11.23**, UUID **1.23.1 → 1.27.0**, thiserror **2.0.18 → 2.0.21**,
      flate2 **1.1.9 → 1.1.10**, and log **0.4.29 → 0.4.34**. Inspect affected
      indirect updates and platform/security requirements. Keep all existing
      warnings-as-errors and focused regression gates. Source:
      [crates.io version metadata](https://crates.io/).
- [ ] **UPG22 — Check the remaining security surfaces.** Run pinned native
      RustSec/Cargo, Go vulnerability, and container OS/browser-layer scanners
      through mise. Record their exact dependency/image scope, findings, and
      remediation; JS registry audit success does not cover these surfaces.

### Already current or deliberately retained

No version bump is presently needed for Node **24.21.0** (latest LTS; 26 is
still Current), mise **2026.10.3**, npm **12.2.0**, pnpm **12.10.1**, k6 **2.3.0**,
Woodpecker CLI **3.19.0**, golangci-lint **2.14.0**, Playwright **1.63.0**, or the
OpenAPI JS tools (**openapi-typescript 7.13.0**, **openapi-fetch 0.17.0**,
**openapi-react-query 0.5.4**). Sources:
[Node LTS policy](https://nodejs.org/en/about/previous-releases),
[mise releases](https://github.com/jdx/mise/releases),
[npm registry metadata](https://registry.npmjs.org/), and the respective upstream
release pages.

The 2026-10-07 local Compose repair refreshed pnpm from 12.9.1 to 12.10.1,
including mise and generated package-manager metadata. Application Dockerfiles
and Compose consume mise's required build inputs. `pins:sync` regenerates pnpm
metadata; `pins:check` rejects manifest drift in local UI checks, prek, and CI.
The Woodpecker CLI pin was refreshed to
[3.19.0](https://github.com/woodpecker-ci/woodpecker/releases/tag/v3.19.0) after
its update warning surfaced during validation. The
development containers were rebuilt with the configured toolchain; separate
Next.js cache and pnpm store mounts prevent host/container manifest collisions
and dependency hard links across filesystems. API health, worker startup,
migration completion, and UI HTTP 200 responses were verified locally.

### Evidence and completion

The audit read application locks and queried 53 direct Rust package names,
native `pnpm outdated --format=json`, `npm outdated --json`, both JS package
audits, and `go list -mod=readonly -m -u -json all`. The renderer's npm audit
reported zero advisories; this does not scan its OS/browser layers. Upstream
release policies establish support status; availability of a newer library
alone does not establish that an older library is unsupported.

For each checked-off task, record the selected versions, owning file changes,
compatibility decisions, and actual verification. Run affected mise lint,
formatter/type checks, builds, meaningful unit tests, and relevant hooks; keep
database-server checks separate and opt-in. Image changes need startup and
affected behavior checks, not just Dockerfile syntax. Deployment completion
requires observed images/versions, readiness, migrations and data/backup checks
where applicable. Keep implementation and tests in one feature commit, and
publish only after explicit feature sign-off. Do not create custom audit or
version-sync scripts to implement this checklist.

## Deferred work

Unchecked deferred work is outside the current queue. Revisit it when its trigger
occurs; deferral does not assert that a check passed.

| ID       | Action                                                                                                                     | Revisit when                                                                                                                         |
| -------- | -------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| DATA09   | Reduce large route/chart materialization in previews and reports; preserve the full scoped corpus needed for calculations. | Activity lists or reports show measured memory pressure.                                                                             |
| DATA13   | Improve duplicate detection across mirrored provider activities.                                                           | A real Garmin/Strava duplicate is observed; preserve provider identity and replayable source data.                                   |
| DATA14   | Preserve useful FIT developer fields and MTB dynamics in normalized storage.                                               | A requested display or analytics feature needs grit, flow, jumps, hang time, or jump distance.                                       |
| DATA16   | Measure whether segment-route deduplication needs a compact stored key.                                                    | A measured route-index or matching cost shows the current representation is too expensive.                                           |
| DATA15   | Reduce repeated single-activity parsing and retained copies.                                                               | A single import shows memory or parsing latency after query/lifetime fixes.                                                          |
| DATA17   | Store provider correlation IDs independently of filenames and formats.                                                     | A source migration or replay change needs stable IDs across artifact renames.                                                        |
| ACT03    | Keep unavailable telemetry distinct from parse failure and list payloads compact.                                          | Riders encounter ambiguous activity details or measurements show list payload pressure.                                              |
| ACT04    | Guide riders toward FIT and distinguish original, provider, and generated artifact downloads.                              | Riders need clearer format guidance or request provider-payload/export downloads.                                                    |
| AUTH05   | Resolve event-template storage and Strava insufficient-scope recovery guidance.                                            | A user requests event templates or a reconnect is blocked by missing provider scopes.                                                |
| OPS02    | Keep admin task responses consistent and verify authorization and safe event metadata.                                     | An admin operation or event field is added or changed.                                                                               |
| SEG01    | Complete visible segment-job progress and focused enqueue/worker/UI coverage.                                              | Riders need task lifecycle visibility or a segment-processing change exposes a gap.                                                  |
| TRAIN01  | Expand focused XC/DH analytics coverage and evaluate additional training context.                                          | A calculation regression or a specific product question justifies the relevant slice.                                                |
| XC01     | Extend event-readiness guidance with stop budget and event/route specificity.                                              | A real target event or product request needs more specific guidance.                                                                 |
| RACE01   | Resolve race-viewer speed and zoom persistence choices and playback edge behavior.                                         | Rider feedback or a playback regression requires a preference or camera change.                                                      |
| REC03    | Rehearse a failed migration and forward repair in isolation.                                                               | A migration fails or a planned schema change needs a repair rehearsal.                                                               |
| REC04    | Rehearse previous-image rollback with backend/UI pins and schema compatibility.                                            | A release needs rollback or compatibility becomes uncertain.                                                                         |
| STRAVA09 | Verify receiver outage, retry/dead-letter, and selective replay.                                                           | Deliveries remain stuck despite the existing retry path.                                                                             |
| STRAVA10 | Verify create/update/delete/deauthorization and backfill effects.                                                          | Those workflows change or fail during ordinary use.                                                                                  |
| LATER02  | Measure startup/idle memory, image size, and one request/import workload.                                                  | Host resource pressure or throughput needs a measurement.                                                                            |
| LATER06  | Define host, size, and time policy for supplied archive URLs.                                                              | Use expands beyond trusted owners or an import exceeds practical limits.                                                             |
| LATER08  | Compare bounded import finalization with Rust's batch behavior.                                                            | An actual large sync/backlog shows a finalization bottleneck.                                                                        |
| LATER09  | Publish image-scan and SBOM artifacts through the existing component pipelines.                                            | Image-scan/SBOM publication becomes a requested release requirement.                                                                 |
| LATER10  | Expand tracing, sampling, dashboard, or scrape validation for an observed visibility problem.                              | A missing trace, misleading panel, scrape problem, or Collector/scaling change needs diagnosis.                                      |
| LATER11  | Revisit report analyzer, filters, completeness, and test expansion.                                                        | A report bug or request justifies resolving the spec's route-family, stopped-time, coasting, power, UI-boundary, or caching choices. |
| LATER12  | Figure out how to obtain and use Strava webhook signature secrets.                                                         | Strava documents a supported signing-key mechanism or its API team provides actionable provisioning guidance.                        |
