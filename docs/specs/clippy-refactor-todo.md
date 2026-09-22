# Clippy Refactor Todo

Bike denies these Clippy lints at the workspace level:

- `clippy::cognitive_complexity`
- `clippy::too_many_arguments`
- `clippy::too_many_lines`

Thresholds live in `clippy.toml`. Existing exceptions must stay local with a reason and should be removed when the owning code is refactored. Do not add broad module or crate allows for new code.

## Refactor Rules

- Prefer idiomatic Rust patterns over habits imported from other languages.
- Do not replace long argument lists with generic parameter bags. In Rust, prefer extracting cohesive domain state only when the grouped values have a real lifecycle, invariant, or behavior. Other good fixes include moving behavior onto the owning type, introducing a request/options struct for a true API boundary, using a builder for optional configuration, or creating small typed values/newtypes when primitive arguments are ambiguous.
- Split orchestration from stage behavior when a function sequences multiple pipeline steps.
- Move repeated response-building or metric-building logic behind named helpers.
- For tests, prefer reusable fixtures, builders, or small test-data modules once the same setup appears more than twice. Keep important per-test differences visible at the call site. Use mocks mainly at external-effect or trait boundaries; otherwise prefer concrete fixtures, in-memory fakes, or real domain values with sensible defaults.
- Remove a Clippy exception in the same change that brings the function under the workspace threshold.

## Analytics And Infrastructure

- `api/src/analytics.rs`: `rebuild_fitness_freshness_cache` allows `too_many_lines`.
  - Todo: extract date-window loading, daily row generation, and stale-state cleanup.
- `api/src/analytics.rs`: `rebuild_segment_analytics_cache` allows `too_many_lines`.
  - Todo: extract segment effort aggregation and analytics row persistence.
- `api/src/analytics.rs`: `rebuild_activity_analytics_cache` allows `too_many_lines`.
  - Todo: extract activity metric aggregation and cache upsert behavior.
- `api/src/provider_rate_limit.rs`: `reserve_provider_quota` allows `too_many_lines`.
  - Todo: isolate bucket loading, reset calculation, reservation decision, and writeback.
- `api/src/observability.rs`: `init_observability` allows `cognitive_complexity`.
  - Todo: split log layer, OTLP exporter, tracer provider, and fallback configuration.
- `migration/src/lib.rs`: crate-level migration registration allows `too_many_lines`.
  - Todo: evaluate whether migration registration can be grouped without hiding migration order.

## Route Tables, Admin, And Response Fixtures

- `api/src/controllers/mod.rs`: `routes` allows `too_many_lines`.
  - Todo: consider grouped route registration helpers by domain.
- `api/src/controllers/admin.rs`: `bike_metrics` allows `too_many_lines`.
  - Todo: group metrics by activity, import, segment, provider, and report domains.
- `api/src/controllers/admin.rs`: `bike_metrics_returns_expected_named_stats` allows `too_many_lines`.
  - Todo: use expected-stat builders or grouped assertions.
- `api/src/controllers/activities.rs`: `load_activity_segment_efforts_by_activity_ids` allows `too_many_lines`.
  - Todo: split effort loading, segment loading, and response grouping.
- `api/src/controllers/activities.rs`: `activity_response_maps_model_fields` allows `too_many_lines`.
  - Todo: use model and response fixture builders.
- `api/src/controllers/activity_imports.rs`: `activity_import_response_maps_model_fields` allows `too_many_lines`.
  - Todo: use import model and event fixture builders.
