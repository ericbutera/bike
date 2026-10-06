# Bike backlog

This is the active checklist. [Product specifications](../bike-rs/docs/specs/README.md)
define behavior. Completed migration and deployment records remain in Git.
Keep private activity data out of committed reports and fixtures.

## Active work

| ID     | State                | Action                                                                         | Completion criteria                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------ | -------------------- | ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| DATA04 | Pending verification | Bound archive working sets and format preference.                              | Verify file-backed descriptors, one bounded expanded member, FIT/TCX/GPX preference, retained partial progress/errors, and peak memory on an existing archive fixture.                                                                                                                                                                                                                                                                                                                             |
| DATA05 | Pending verification | Match the current import and refresh affected analytics.                       | Include shared segments in bounded pages; refresh displaced activities and old/new segment summaries; preserve per-activity effort numbering through regeneration and sport changes. Verify workflow, SQL, and working set.                                                                                                                                                                                                                                                                        |
| DATA06 | Pending verification | Separate training inputs from report summaries and bound backfills.            | Read complete ordered samples for one owned ride after matching; backfill scalar IDs without retaining whole-history routes. Verify owning behavior, SQL, and memory.                                                                                                                                                                                                                                                                                                                              |
| DATA18 | Awaiting originals   | Recover authentic sources for remaining generated Strava TCX records.          | [Recovery contract](../bike-rs/docs/specs/activity-ingestion.md#generated-strava-tcx-retirement-and-source-backfill): obtain missing originals from an updated archive or explicitly retire those sources; replay and verify eligible outdoor controls and zero virtual contributions. Preserve summaries/GPS while sources remain unavailable.                                                                                                                                                    |
| DATA19 | Proposed             | Separate normalized GPS/chart/lap data from activity summaries.                | [Storage proposal](../bike-rs/docs/specs/activity-ingestion.md#proposed-separation-of-activity-detail-from-summaries): owned one-to-one activity details, lightweight recording/admission decisions, bounded verified backfill and transactional replay/deletion, explicit detail reads, and measured bytes/latency/memory before considering per-point or chunked storage. No schema migration implemented.                                                                                       |
| DATA12 | Pending              | Recover imports across checkpoint, lock, queue, and replay boundaries.         | Retry recovers one linked activity and downstream work; live locks remain held, stale locks/orphans recover, contention is retryable, and retained source/errors survive bounded replay. Includes REC05.                                                                                                                                                                                                                                                                                           |
| ACT02  | Pending verification | Confirm Garmin FIT sub-sport classification and targeted archive reprocessing. | Record importer behavior and production recovery for an existing relevant FIT recording.                                                                                                                                                                                                                                                                                                                                                                                                           |
| ACT05  | Pending              | Retain non-cycling archive/upload inputs without ride processing.              | [Retention contract](../bike-rs/docs/specs/activity-ingestion.md#non-cycling-retention-proposal): owned originals and minimal summaries, explicit deferred outcome, zero full GPS/detail decodes or downstream ride jobs, idempotent future promotion, and measured mixed-sport import costs. Reuse import/artifact storage. Existing gateway delivers no non-cycling summary; gateway changes are outside scope.                                                                                  |
| MAPS12 | Pending              | Require explicit heatmap admission and withhold unknown recordings.            | [Admission proposal](../bike-rs/docs/specs/activity-ingestion.md#heatmap-admission-proposal): agree supported outdoor evidence rules and their trust limits; persist a versioned decision separate from claimed environment; enforce all ingestion, replay, preparation, publication, and read boundaries; positive outdoor and unknown/virtual controls; two-user isolation; migrations/backfill and live verification before second-user imports. This stricter policy is not implemented by v5. |
| MAPS13 | Pending              | Define a separate global cycling heatmap after personal admission is verified. | [Scope](specs/heatmaps.md#cycling-admission-proposal-2026-10-06): explicit participation, stricter contribution admission, separate aggregate queries/revisions, removal on opt-out/deletion/reclassification, no automatic global approval from personal overrides, and two-owner correctness tests. No global endpoint is implemented.                                                                                                                                                           |
| GEO02  | Pending              | Measure vanilla/PostGIS data access and resource costs.                        | Current and projected vanilla controls versus extension-only/spatial PostGIS use identical fixtures; segments, heatmaps, map rendering, activity detail, and race viewer have correctness, stage timings, CPU, memory, storage and write-cost evidence before adopting a spatial design.                                                                                                                                                                                                           |
| TEST10 | Implemented locally  | Separate synthetic availability monitoring from browser e2e.                   | Separate k6 availability and Playwright images build and pass fixture checks. Production publication and runtime verification remain separate from local implementation.                                                                                                                                                                                                                                                                                                                           |

## System upgrade checklist

Version audit checked on **2026-10-06**. This is planned work; no upgrades or
deployments were performed by this audit. Targets are released stable versions
verified from upstream policies and registries. Refresh their patch versions
before implementation. Prefer the latest supported LTS line where one exists;
Rust, Go, npm, pnpm, and most libraries do not have a Node-style LTS channel.

Scope: root/component mise configs, application manifests and lockfiles, all
tracked Dockerfiles, Compose, Woodpecker, code generators, the vendored Rust
patch, and the retained PostGIS experiment. Infrastructure definitions were
checked only for the database baseline; running production versions, host
software, image digests, and container OS package vulnerabilities still require
deployment inspection. This checklist does not certify the absence of unused
code or documentation.

### Security and unsupported software first

- [ ] **UPG01 — Patch Next.js and its production dependencies.**
      `bike-ui/package.json` and the lockfile pin Next/ESLint config **16.1.6**;
      the registry's current Active-LTS release is **16.4.0**. Upgrade Next and
      `eslint-config-next` together. Move React/React DOM **19.2.3 → 19.3.0**
      together after checking compatibility. The installed Next version is affected
      by the [AVIF image optimization advisory](https://github.com/vercel/next.js/security/advisories/GHSA-2xp9-vwfh-vxw4),
      fixed from 16.3.3; that is a minimum security fix, not the final target.
      Update the lockfile's `sharp` **0.34.5**, PostCSS **8.4.31/8.5.14**, and
      `nanoid` **3.3.12** through supported parent releases. Verify image rendering,
      auth/proxy behavior, server rendering, and the production UI build.
      Sources: [Next support policy](https://nextjs.org/support-policy),
      [Next registry](https://registry.npmjs.org/next/latest),
      [React registry](https://registry.npmjs.org/react/latest).
- [ ] **UPG02 — Unify supported MapLibre versions.** UI **5.24.0** and renderer
      **6.11.2** differ; target **6.13.0** in both after migration review. The UI
      version falls within the [sanitizer advisory](https://github.com/maplibre/maplibre-gl-js/security/advisories/GHSA-jrc7-96c5-q579)
      affected range; the fix starts at 6.4.1. Verify activity/segment/race maps,
      personal heatmap controls, attribution, tiles, and renderer output. Preserve
      outdoor controls and virtual/GPS-gap exclusions. Source:
      [MapLibre registry](https://registry.npmjs.org/maplibre-gl/latest).
- [ ] **UPG03 — Replace the unsupported Go toolchain.** Root mise and the
      gateway Dockerfile use **1.25.1**; target **1.27.1**. Go supports only its
      two newest release lines, currently 1.26 and 1.27. Review the gateway's
      `go.mod` minimum language version separately from the build toolchain, and
      verify the pinned compiler is used without an automatic toolchain substitution.
      Run gateway lint/tests/build and protobuf freshness with the new toolchain;
      verify linux/amd64 images. Source:
      [Go releases and support policy](https://go.dev/doc/devel/release).
- [ ] **UPG04 — Move UI ESLint off its EOL major.** UI **9.39.5 → 10.12.0**;
      renderer already uses 10.12.0. ESLint 9 ended upstream maintenance on
      2026-08-06. Coordinate with UPG01's Next config and plugin peer requirements;
      retain all strict rules and zero-warning enforcement. Source:
      [ESLint support policy](https://eslint.org/version-support/).
- [ ] **UPG05 — Replace Jaeger 1 in local tracing.** Root mise and Rust Compose
      use `jaegertracing/all-in-one:latest`, the retired v1 image family. Jaeger 1
      reached EOL on 2025-12-31. Move to a pinned **Jaeger 2.22.0** image and its
      supported configuration; verify OTLP ingestion, trace lookup, ports, and the
      existing optional tracing profile. Coordinate any separately owned IaC
      deployment instead of assuming Compose changes update production.
      Sources: [Jaeger lifecycle](https://www.jaegertracing.io/download/),
      [2.22.0 release](https://github.com/jaegertracing/jaeger/releases/tag/v2.22.0).
- [ ] **UPG06 — Replace unmaintained cargo-watch.** Both Rust dev Dockerfiles
      install **8.5.3**, whose upstream is archived and no longer receives updates.
      Use maintained [watchexec](https://github.com/watchexec/watchexec) through
      mise (current CLI **2.8.0**) or another supported existing watch workflow.
      Verify API/worker reload, shutdown signals, and rebuild failure output.
      Remove the obsolete pin and installation together. Source:
      [cargo-watch maintenance statement](https://github.com/watchexec/cargo-watch#maintenance).
- [ ] **UPG07 — Resolve the remaining UI audit findings.** `pnpm audit --json`
      reported **3 critical, 23 high, 20 moderate, 6 low** findings in the current
      installed dependency tree, including development dependencies. These are
      dependency matches, not a finding that every issue is exploitable in Bike.
      Besides UPG01/02, affected paths include Vitest **4.1.5** (fix ≥4.1.11),
      Vite **8.0.10** (fix ≥8.0.16), `happy-dom → ws` **8.20.0** (fix ≥8.21.0),
      `mermaid → dompurify` **3.4.13** (fix ≥3.4.16), KaTeX (fix ≥0.18.2),
      `source-map-js` **1.2.1** (fix ≥1.2.2), and
      `eslint-config-next → fast-glob → micromatch → braces` **3.0.3**, for which
      the audit reports no patched version. Upgrade supported parents, investigate
      the remaining path, and verify the refreshed production and development
      dependency graphs. Do not blanket-force fixes or suppress findings.
      Record RustSec and Go vulnerability scans as separate remaining checks;
      this audit did not execute those scanners or scan container layers.

### Toolchains, database, and images

- [ ] **UPG08 — Use current stable Rust consistently.** Root/application
      **1.97.1 → 1.99.0**; PostGIS's separate mise config still pins **1.93.0**.
      Update the shared toolchain, Rust Docker defaults/builder image, Clippy and
      rustfmt together. Have the experiment inherit the shared pin or document a
      deliberate frozen benchmark exception. Its digest-pinned compiler/runtime
      must be identified before changing it; regenerate comparable measurements
      rather than comparing different environments as equivalent. Rust has no LTS
      release line. Source:
      [Rust 1.99.0 announcement](https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/).
- [ ] **UPG09 — Align and patch PostgreSQL before a major migration.** Local
      Bike uses floating **17**, gateway fixtures floating **16**, and the
      infrastructure definition/backup image pins **17.7-trixie**. Both majors
      remain supported; latest patches are **17.11** and **16.15**. First verify
      the running versions and move to a consistent, explicitly pinned **17.11**
      baseline across application development, optional gateway tests, production,
      and backup/restore tooling. Keep real database checks opt-in rather than
      adding a CI server for unit tests. Source:
      [PostgreSQL version/support policy](https://www.postgresql.org/support/versioning/).
- [ ] **UPG10 — Rehearse PostgreSQL 18 before changing persistent instances.**
      Latest released major is **18.6**; PostgreSQL 19 is still prerelease in the
      checked upstream release index. Validate dump/restore or `pg_upgrade`,
      backups, extensions, database queries, and Docker data-directory/volume layout.
      Preserve a tested recovery route and coordinate with the owning IaC repo.
      Do not reuse a 17 data directory by changing its image tag alone.
      Sources: [PostgreSQL releases](https://www.postgresql.org/docs/release/),
      [official image upgrade notes](https://github.com/docker-library/docs/blob/master/postgres/README.md).
- [ ] **UPG11 — Refresh and pin base images.** Rust and gateway builds/runtimes,
      browser e2e, the renderer's Node stage, and CI bootstrap use Debian
      **12/bookworm**. It is now in LTS; **13/trixie** is current stable. Review
      compatible official image variants, libc/OpenSSL package names, CA certificates,
      fonts/browser dependencies, and image size before moving. The renderer's
      Playwright **noble** base is already Ubuntu 24.04 LTS; preserve its matching
      browser/library version. UI `node:24.21.0-alpine` leaves the Alpine release
      implicit: identify and pin the intended maintained variant/digest. Replace
      floating cargo-chef, plugin, database, and `latest` tags with reviewed release
      identities. Source: [Debian lifecycle](https://www.debian.org/releases/).
- [ ] **UPG12 — Refresh quality-tool pins.** Root Prettier **3.7.4 → 3.9.9**
      and prek **0.4.12 → 0.5.5**. Review native config changes, format the affected
      files, validate `prek.toml`, reinstall/check hooks, and keep CI on the owning
      mise tasks. Sources: [Prettier registry](https://registry.npmjs.org/prettier/latest),
      [prek releases](https://github.com/j178/prek/releases).
- [ ] **UPG13 — Align protobuf generators with runtimes.** Gateway
      `protoc-gen-go` **1.36.6 → 1.36.12** (runtime already 1.36.12), and
      `protoc-gen-go-grpc` **1.5.1 → 1.6.2**. Regenerate and inspect binding changes,
      then run native freshness and gateway/receiver fixture checks. Rust
      `protoc-bin-vendored` locks **3.2.0** while **3.3.0** is available and bypasses
      mise's protoc selection in `build.rs`; choose one explicit compiler policy.
      Root protoc **36.2**, Prost **0.14.4**, and Tonic **0.14.6** are already
      current in their checked registries. Sources:
      [Go protobuf](https://proxy.golang.org/google.golang.org/protobuf/@latest),
      [Go gRPC generator](https://proxy.golang.org/google.golang.org/grpc/cmd/protoc-gen-go-grpc/@latest),
      [vendored protoc](https://crates.io/crates/protoc-bin-vendored).
- [ ] **UPG14 — Make mise pins reach every consumer.** Woodpecker's image
      build steps currently use Dockerfile defaults rather than passing root mise
      vars; Compose and local image tasks pass only some build arguments. Upgrade
      owning tasks, supported plugin inputs, Docker defaults, package-manager
      metadata, hooks, and documentation together. Decide whether to retain an
      unmodified upstream mise bootstrap or use a pinned mise CI image; avoid
      maintaining custom installer changes or another version-parser script.
      Pin cargo-chef itself (current release **0.1.78**) rather than only a
      `latest-rust-*` tag. Identify/pin Woodpecker's clone/build plugin images:
      upstream Google Kaniko is archived, but the
      [Woodpecker plugin now uses a maintained fork](https://github.com/woodpecker-ci/plugin-kaniko).
      Verify the selected image actually contains that fork before planning a
      builder replacement. Source:
      [mise CI options](https://mise.jdx.dev/continuous-integration.html).

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
      regression fixtures. FIT parser **0.11.0** is current. The PostGIS probe also
      uses PNG 0.17 and SHA-2 0.10; include it or retain an explicit experiment baseline.
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
      default; TypeScript **5.9.3 → 7.0.2**, Vitest **4.1.5 → 5.0.3**, Vite React
      plugin **6.0.1 → 6.1.2**, jest-dom **6.9.1 → 7.0.1**, happy-dom
      **20.9.0 → 20.14.5**, and Mermaid **11.16.1 → 12.1.0**. Review TypeScript
      6/7 migration changes and test/plugin peers; verify meaningful unit assertions,
      generated types, rendering, diagrams, and fake-boundary browser checks.
      Remaining smaller updates: Tailwind/PostCSS plugin **4.2.4 → 4.3.3**,
      React Query **5.100.9 → 5.104.1**, DaisyUI **5.7.37 → 5.7.47**, Recharts
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

### Already current or deliberately retained

No version bump is presently needed for Node **24.21.0** (latest LTS; 26 is
still Current), mise **2026.10.3**, npm **12.2.0**, pnpm **12.9.1**, k6 **2.3.0**,
Woodpecker CLI **3.18.1**, golangci-lint **2.14.0**, Playwright **1.63.0**, or the
OpenAPI JS tools (**openapi-typescript 7.13.0**, **openapi-fetch 0.17.0**,
**openapi-react-query 0.5.4**). Sources:
[Node LTS policy](https://nodejs.org/en/about/previous-releases),
[mise releases](https://github.com/jdx/mise/releases),
[npm registry metadata](https://registry.npmjs.org/), and the respective upstream
release pages. PostGIS **3.6.4** is the latest checked stable series; 3.7 is
still prerelease. Keep the experiment's reproducible package/digest controls
until its baseline is intentionally refreshed.
Source: [PostGIS releases](https://postgis.net/news/).

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
