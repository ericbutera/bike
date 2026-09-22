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

## Reports And Training Goals

- `api/src/controllers/reports.rs`: `build_training_reports` allows `too_many_lines`.
  - Todo: move report dispatch and report-specific loading behind smaller report builders.
- `api/src/controllers/reports.rs`: `build_aggregate_training_reports` allows `too_many_lines`.
  - Todo: extract boundary/range loading and aggregate report assembly.
- `api/src/controllers/reports.rs`: `report_definitions` allows `too_many_lines`.
  - Todo: consider a static definition table or grouped definition builders.
- `api/src/controllers/reports.rs`: `build_ride_summary_report` allows `too_many_lines`.
  - Todo: extract totals, averages, activity-type distribution, and data-quality sections.
- `api/src/controllers/reports.rs`: `build_compare_rides_report` allows `too_many_lines`.
  - Todo: extract selected ride metrics and candidate row mapping.
- `api/src/controllers/reports.rs`: `build_reassessment_report` allows `too_many_lines`.
  - Todo: split window selection, metric generation, benchmark selection, and response assembly.
- `api/src/controllers/reports.rs`: `reassessment_window_metrics` allows `too_many_lines`.
  - Todo: extract volume, climbing, fitness, and benchmark calculations.
- `api/src/controllers/reports.rs`: `reassessment_climbing_density_signal` allows `too_many_lines`.
  - Todo: extract evidence scoring and threshold comparison.
- `api/src/controllers/reports.rs`: `reassessment_signal` allows `too_many_arguments`.
  - Todo: replace optional signal fields with a builder or detail struct.
- `api/src/controllers/reports.rs`: `detect_climbs` allows `too_many_lines`.
  - Todo: extract climb candidate state and summit/finalization rules.
- `api/src/controllers/reports.rs`: `finalize_climb` allows `too_many_arguments`.
  - Todo: introduce a climb candidate struct.
- `api/src/controllers/training_goals.rs`: `build_xc_goal_progress_response` allows `too_many_lines`.
  - Todo: split progress rows, readiness summary, and race result sections.
- `api/src/controllers/training_goals.rs`: `build_xc_readiness` allows `too_many_lines`.
  - Todo: extract readiness gate construction and deficit generation.
- `api/src/controllers/training_goals.rs`: `build_at_least_gate` allows `too_many_arguments`.
  - Todo: replace gate parameters with a readiness gate input struct.
- `api/src/controllers/training_goals.rs`: `build_at_most_gate` allows `too_many_arguments`.
  - Todo: reuse the readiness gate input struct.
- `api/src/controllers/training_goals.rs`: `build_xc_race_results` allows `too_many_lines`.
  - Todo: extract race grouping and result row mapping.

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
