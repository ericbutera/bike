# Happy-path unit coverage checklist

Baseline captured on 2026-10-09 from the first local Rust and Next.js JSON reports.
Only files with executable lines and **0% line coverage** are listed. Rust
`migration/**` is excluded. The initial reports included the existing native
SQLite API suite; new work below uses unit tests only, with mocked boundaries.
No integration or E2E tests are added or run for this checklist.

A checked item means a meaningful happy-path unit test was added and passed,
and a fresh report records executed lines. It does not mean 100% coverage.
Unchecked items remain visible for future work. Runtime entrypoints and worker
orchestration need isolated boundaries before they can be tested with small
units; do not execute live services merely to check off a file.

Fresh reports on 2026-10-09 were collected with `mise run coverage:unit`:

| Suite   | New happy-path tests | Paths with new coverage | Paths still at 0% | Overall line coverage |
| ------- | -------------------- | ----------------------- | ----------------- | --------------------- |
| Rust    | 12                   | 10                      | 49                | 57.55%                |
| Next.js | 13                   | 11                      | 43                | 58.70%                |

After integration with current main, the full unit run passed 294 Rust tests
(three ignored) and 214 Next.js tests.
Rust totals exclude migrations and integration execution, so they are not directly
comparable with the original full-workspace baseline. Separate Rust test files
are excluded from the report. The paths below include every file still at 0%
in the current unit reports; checked entries retain the original gaps for tracking.

Reports are generated locally under `.artifacts/coverage/{rust,nextjs}/`, with
HTML in `html/index.html` and per-file data in `coverage-summary.json`.
CI enforces [80% coverage for changed lines](development.md#ci-coverage-policy-and-viewing-reports)
while keeping these existing gaps exempt. Published main reports are available
on the free [coverage site](https://ericbutera.github.io/bike/coverage/).

## Rust: 45 initially uncovered files

- [ ] [bike-rs/api/src/activity_import_lock.rs](../bike-rs/api/src/activity_import_lock.rs)
- [ ] [bike-rs/api/src/bin/openapi.rs](../bike-rs/api/src/bin/openapi.rs)
- [ ] [bike-rs/api/src/controllers/heatmaps.rs](../bike-rs/api/src/controllers/heatmaps.rs)
- [ ] [bike-rs/api/src/main.rs](../bike-rs/api/src/main.rs)
- [ ] [bike-rs/api/src/segment_support.rs](../bike-rs/api/src/segment_support.rs)
- [x] [bike-rs/bike-core/src/activity_achievements.rs](../bike-rs/bike-core/src/activity_achievements.rs) — 100.00% lines; stored achievement round-trip and supported unversioned data ([tests](../bike-rs/bike-core/src/activity_achievements_tests.rs)).
- [ ] [bike-rs/bike-core/src/auth/entities/api_clients.rs](../bike-rs/bike-core/src/auth/entities/api_clients.rs)
- [ ] [bike-rs/bike-core/src/auth/entities/cooldowns.rs](../bike-rs/bike-core/src/auth/entities/cooldowns.rs)
- [ ] [bike-rs/bike-core/src/background_jobs/worker/config.rs](../bike-rs/bike-core/src/background_jobs/worker/config.rs)
- [x] [bike-rs/bike-core/src/background_jobs/worker/metrics.rs](../bike-rs/bike-core/src/background_jobs/worker/metrics.rs) — 78.69% lines; completed-task counts and timings; no metrics server ([tests](../bike-rs/bike-core/src/background_jobs/worker/metrics_tests.rs)).
- [ ] [bike-rs/bike-core/src/background_jobs/worker/processor.rs](../bike-rs/bike-core/src/background_jobs/worker/processor.rs)
- [ ] [bike-rs/bike-core/src/background_jobs/worker/scheduler.rs](../bike-rs/bike-core/src/background_jobs/worker/scheduler.rs)
- [ ] [bike-rs/bike-core/src/background_jobs/worker/task_worker.rs](../bike-rs/bike-core/src/background_jobs/worker/task_worker.rs)
- [ ] [bike-rs/bike-core/src/background_jobs/worker/tracing.rs](../bike-rs/bike-core/src/background_jobs/worker/tracing.rs)
- [ ] [bike-rs/bike-core/src/db.rs](../bike-rs/bike-core/src/db.rs)
- [ ] [bike-rs/bike-core/src/entities/activity_archive_import_jobs.rs](../bike-rs/bike-core/src/entities/activity_archive_import_jobs.rs)
- [ ] [bike-rs/bike-core/src/heatmaps/data.rs](../bike-rs/bike-core/src/heatmaps/data.rs)
- [x] [bike-rs/bike-core/src/heatmaps/types.rs](../bike-rs/bike-core/src/heatmaps/types.rs) — 93.33% lines; valid sport/date filters and cache identity ([tests](../bike-rs/bike-core/src/heatmaps/types_tests.rs)).
- [ ] [bike-rs/bike-core/src/jobs/email.rs](../bike-rs/bike-core/src/jobs/email.rs)
- [ ] [bike-rs/bike-core/src/platform/auth_metrics.rs](../bike-rs/bike-core/src/platform/auth_metrics.rs)
- [ ] [bike-rs/bike-core/src/platform/background_task_metrics.rs](../bike-rs/bike-core/src/platform/background_task_metrics.rs)
- [x] [bike-rs/bike-core/src/platform/cooldown.rs](../bike-rs/bike-core/src/platform/cooldown.rs) — 6.77% lines; capped backoff calculation only; database leases remain untested ([tests](../bike-rs/bike-core/src/platform/cooldown_tests.rs)).
- [x] [bike-rs/bike-core/src/platform/data/sorting.rs](../bike-rs/bike-core/src/platform/data/sorting.rs) — 87.50% lines; supported orders and query construction; no database ([tests](../bike-rs/bike-core/src/platform/data/sorting_tests.rs)).
- [x] [bike-rs/bike-core/src/platform/email/service.rs](../bike-rs/bike-core/src/platform/email/service.rs) — 56.96% lines; message bodies, sender/recipient, and delivery identity; no SMTP send ([tests](../bike-rs/bike-core/src/platform/email/service_tests.rs)).
- [x] [bike-rs/bike-core/src/platform/email/templates.rs](../bike-rs/bike-core/src/platform/email/templates.rs) — 100.00% lines; registered template rendering ([tests](../bike-rs/bike-core/src/platform/email/templates_tests.rs)).
- [x] [bike-rs/bike-core/src/platform/error.rs](../bike-rs/bike-core/src/platform/error.rs) — 93.75% lines; error response status and public message ([tests](../bike-rs/bike-core/src/platform/error_tests.rs)).
- [ ] [bike-rs/bike-core/src/platform/system_metrics.rs](../bike-rs/bike-core/src/platform/system_metrics.rs)
- [ ] [bike-rs/bike-core/src/segment_regeneration.rs](../bike-rs/bike-core/src/segment_regeneration.rs)
- [x] [bike-rs/bike-core/src/services/cooldown.rs](../bike-rs/bike-core/src/services/cooldown.rs) — 26.58% lines; rider report-generation policy only; database operations remain untested ([tests](../bike-rs/bike-core/src/services/cooldown_tests.rs)).
- [x] [bike-rs/worker/src/email/templates.rs](../bike-rs/worker/src/email/templates.rs) — 88.24% lines; text and HTML notification rendering ([tests](../bike-rs/worker/src/email/templates_tests.rs)).
- [ ] [bike-rs/worker/src/main.rs](../bike-rs/worker/src/main.rs)
- [ ] [bike-rs/worker/src/tasks/mod.rs](../bike-rs/worker/src/tasks/mod.rs)
- [ ] [bike-rs/worker/src/tasks/processors/activity_archive_import.rs](../bike-rs/worker/src/tasks/processors/activity_archive_import.rs)
- [ ] [bike-rs/worker/src/tasks/processors/backfill_user_xc_training.rs](../bike-rs/worker/src/tasks/processors/backfill_user_xc_training.rs)
- [ ] [bike-rs/worker/src/tasks/processors/email_notification.rs](../bike-rs/worker/src/tasks/processors/email_notification.rs)
- [ ] [bike-rs/worker/src/tasks/processors/prepare_heatmap.rs](../bike-rs/worker/src/tasks/processors/prepare_heatmap.rs)
- [ ] [bike-rs/worker/src/tasks/processors/process_activity_import.rs](../bike-rs/worker/src/tasks/processors/process_activity_import.rs)
- [ ] [bike-rs/worker/src/tasks/processors/rebuild_fitness_freshness.rs](../bike-rs/worker/src/tasks/processors/rebuild_fitness_freshness.rs)
- [ ] [bike-rs/worker/src/tasks/processors/rebuild_segment_analytics.rs](../bike-rs/worker/src/tasks/processors/rebuild_segment_analytics.rs)
- [ ] [bike-rs/worker/src/tasks/processors/regenerate_segment_efforts.rs](../bike-rs/worker/src/tasks/processors/regenerate_segment_efforts.rs)
- [ ] [bike-rs/worker/src/tasks/processors/regenerate_user_segments.rs](../bike-rs/worker/src/tasks/processors/regenerate_user_segments.rs)
- [ ] [bike-rs/worker/src/tasks/processors/reprocess_activity_import.rs](../bike-rs/worker/src/tasks/processors/reprocess_activity_import.rs)
- [ ] [bike-rs/worker/src/tasks/processors/reprocess_user_activity_imports.rs](../bike-rs/worker/src/tasks/processors/reprocess_user_activity_imports.rs)
- [ ] [bike-rs/worker/src/tasks/processors/strava_sync.rs](../bike-rs/worker/src/tasks/processors/strava_sync.rs)
- [ ] [bike-rs/worker/src/tasks/startup.rs](../bike-rs/worker/src/tasks/startup.rs)

### Rust: 14 additional unit-only gaps

These files had executed lines in the original report through the existing
integration suite, but have 0% coverage when only unit tests run. They are included
here so integration coverage does not hide missing unit happy paths.

- [ ] [bike-rs/api/src/controllers/synthetics.rs](../bike-rs/api/src/controllers/synthetics.rs)
- [ ] [bike-rs/api/src/storage.rs](../bike-rs/api/src/storage.rs)
- [ ] [bike-rs/api/src/xc_goal_backfill.rs](../bike-rs/api/src/xc_goal_backfill.rs)
- [ ] [bike-rs/bike-core/src/auth/controllers/oauth.rs](../bike-rs/bike-core/src/auth/controllers/oauth.rs)
- [ ] [bike-rs/bike-core/src/background_jobs/admin.rs](../bike-rs/bike-core/src/background_jobs/admin.rs)
- [ ] [bike-rs/bike-core/src/entities/segment_summaries.rs](../bike-rs/bike-core/src/entities/segment_summaries.rs)
- [ ] [bike-rs/bike-core/src/entities/synthetic_scenarios.rs](../bike-rs/bike-core/src/entities/synthetic_scenarios.rs)
- [ ] [bike-rs/bike-core/src/entities/user_preferences.rs](../bike-rs/bike-core/src/entities/user_preferences.rs)
- [ ] [bike-rs/bike-core/src/platform/feature_flags/admin_controller.rs](../bike-rs/bike-core/src/platform/feature_flags/admin_controller.rs)
- [ ] [bike-rs/bike-core/src/platform/feature_flags/controller.rs](../bike-rs/bike-core/src/platform/feature_flags/controller.rs)
- [ ] [bike-rs/bike-core/src/platform/metrics_controller.rs](../bike-rs/bike-core/src/platform/metrics_controller.rs)
- [ ] [bike-rs/bike-core/src/strava_gateway_metrics.rs](../bike-rs/bike-core/src/strava_gateway_metrics.rs)
- [ ] [bike-rs/bike-core/src/synthetics.rs](../bike-rs/bike-core/src/synthetics.rs)
- [ ] [bike-rs/bike-core/src/xc_goal_backfill.rs](../bike-rs/bike-core/src/xc_goal_backfill.rs)

## Next.js: 54 initially uncovered paths

- [ ] [bike-ui/app/account/page.tsx](../bike-ui/app/account/page.tsx)
- [ ] [bike-ui/app/activities/[id]/page.tsx](../bike-ui/app/activities/[id]/page.tsx)
- [ ] [bike-ui/app/activity-previews/[variant]/[styleVersion]/route.ts](../bike-ui/app/activity-previews/[variant]/[styleVersion]/route.ts)
- [ ] [bike-ui/app/activity-previews/[variant]/route.ts](../bike-ui/app/activity-previews/[variant]/route.ts)
- [ ] [bike-ui/app/admin/activities/page.tsx](../bike-ui/app/admin/activities/page.tsx)
- [ ] [bike-ui/app/admin/analytics/page.tsx](../bike-ui/app/admin/analytics/page.tsx)
- [ ] [bike-ui/app/admin/feature-flags/page.tsx](../bike-ui/app/admin/feature-flags/page.tsx)
- [ ] [bike-ui/app/admin/integrations/page.tsx](../bike-ui/app/admin/integrations/page.tsx)
- [ ] [bike-ui/app/admin/layout.tsx](../bike-ui/app/admin/layout.tsx)
- [ ] [bike-ui/app/admin/manual-tasks/page.tsx](../bike-ui/app/admin/manual-tasks/page.tsx)
- [ ] [bike-ui/app/admin/metrics/page.tsx](../bike-ui/app/admin/metrics/page.tsx)
- [ ] [bike-ui/app/admin/page.tsx](../bike-ui/app/admin/page.tsx)
- [ ] [bike-ui/app/admin/tasks/page.tsx](../bike-ui/app/admin/tasks/page.tsx)
- [ ] [bike-ui/app/admin/users/page.tsx](../bike-ui/app/admin/users/page.tsx)
- [ ] [bike-ui/app/auth/callback/page.tsx](../bike-ui/app/auth/callback/page.tsx)
- [ ] [bike-ui/app/dh/page.tsx](../bike-ui/app/dh/page.tsx)
- [ ] [bike-ui/app/fitness/page.tsx](../bike-ui/app/fitness/page.tsx)
- [ ] [bike-ui/app/layout.tsx](../bike-ui/app/layout.tsx)
- [ ] [bike-ui/app/login/page.tsx](../bike-ui/app/login/page.tsx)
- [ ] [bike-ui/app/logout/page.tsx](../bike-ui/app/logout/page.tsx)
- [ ] [bike-ui/app/maps/page.tsx](../bike-ui/app/maps/page.tsx)
- [ ] [bike-ui/app/page.tsx](../bike-ui/app/page.tsx)
- [ ] [bike-ui/app/segments/[id]/page.tsx](../bike-ui/app/segments/[id]/page.tsx)
- [ ] [bike-ui/app/segments/[id]/race/page.tsx](../bike-ui/app/segments/[id]/race/page.tsx)
- [ ] [bike-ui/app/segments/analysis/page.tsx](../bike-ui/app/segments/analysis/page.tsx)
- [ ] [bike-ui/app/segments/builder/page.tsx](../bike-ui/app/segments/builder/page.tsx)
- [ ] [bike-ui/app/segments/page.tsx](../bike-ui/app/segments/page.tsx)
- [ ] [bike-ui/app/segments/progress/page.tsx](../bike-ui/app/segments/progress/page.tsx)
- [ ] [bike-ui/app/training/reports/layout.tsx](../bike-ui/app/training/reports/layout.tsx)
- [ ] [bike-ui/app/training/reports/page.tsx](../bike-ui/app/training/reports/page.tsx)
- [ ] [bike-ui/app/upload/page.tsx](../bike-ui/app/upload/page.tsx)
- [ ] [bike-ui/app/xc/page.tsx](../bike-ui/app/xc/page.tsx)
- [x] [bike-ui/components/AuthRequiredCard.tsx](../bike-ui/components/AuthRequiredCard.tsx) — 100.00% lines; sign-in invitation and link ([tests](../bike-ui/components/__tests__/AuthRequiredCard.test.tsx)).
- [x] [bike-ui/components/Layout.tsx](../bike-ui/components/Layout.tsx) — 100.00% lines; shared navigation and page content ([tests](../bike-ui/components/__tests__/PageShells.test.tsx)).
- [ ] [bike-ui/components/MapLibreRouteMap.tsx](../bike-ui/components/MapLibreRouteMap.tsx)
- [x] [bike-ui/components/RequireAdmin.tsx](../bike-ui/components/RequireAdmin.tsx) — 64.28% lines; authorized admin content ([tests](../bike-ui/components/__tests__/RequireAdmin.test.tsx)).
- [x] [bike-ui/components/RuntimeConfigScript.tsx](../bike-ui/components/RuntimeConfigScript.tsx) — 100.00% lines; runtime config and escaped request context ([tests](../bike-ui/components/__tests__/RuntimeConfigScript.test.tsx)).
- [ ] [bike-ui/components/SegmentBuilderPage.tsx](../bike-ui/components/SegmentBuilderPage.tsx)
- [ ] [bike-ui/components/SegmentBuilderWorkspace.tsx](../bike-ui/components/SegmentBuilderWorkspace.tsx)
- [ ] [bike-ui/components/SegmentEffortAnalysisReport.tsx](../bike-ui/components/SegmentEffortAnalysisReport.tsx)
- [ ] [bike-ui/components/SegmentYearlyProgressReport.tsx](../bike-ui/components/SegmentYearlyProgressReport.tsx)
- [x] [bike-ui/components/ThemeToggle.tsx](../bike-ui/components/ThemeToggle.tsx) — 97.05% lines; theme selection and persistence ([tests](../bike-ui/components/__tests__/ThemeToggle.test.tsx)).
- [ ] [bike-ui/components/activity-detail/ActivityImportTracePanel.tsx](../bike-ui/components/activity-detail/ActivityImportTracePanel.tsx)
- [ ] [bike-ui/components/admin/AdminUsersPageContent.tsx](../bike-ui/components/admin/AdminUsersPageContent.tsx)
- [ ] [bike-ui/components/admin/BikeMetricsSection.tsx](../bike-ui/components/admin/BikeMetricsSection.tsx)
- [x] [bike-ui/components/admin/LocalAdminLayout.tsx](../bike-ui/components/admin/LocalAdminLayout.tsx) — 100.00% lines; admin content and successful metric display ([tests](../bike-ui/components/__tests__/PageShells.test.tsx)).
- [x] [bike-ui/components/admin/Nav.tsx](../bike-ui/components/admin/Nav.tsx) — 100.00% lines; current-route sidebar selection ([tests](../bike-ui/components/__tests__/PageShells.test.tsx)).
- [x] [bike-ui/components/reports/Charts.tsx](../bike-ui/components/reports/Charts.tsx) — 12.96% lines; UTC year labels only; chart rendering remains untested ([tests](../bike-ui/components/reports/Charts.test.ts)).
- [ ] [bike-ui/components/reports/ReportsClient.tsx](../bike-ui/components/reports/ReportsClient.tsx)
- [ ] [bike-ui/components/reports/ReportsLayout.tsx](../bike-ui/components/reports/ReportsLayout.tsx)
- [x] [bike-ui/components/reports/TimeRangeSelector.tsx](../bike-ui/components/reports/TimeRangeSelector.tsx) — 100.00% lines; interval selection callback ([tests](../bike-ui/components/reports/TimeRangeSelector.test.tsx)).
- [x] [bike-ui/components/reports/reportDefinitions.ts](../bike-ui/components/reports/reportDefinitions.ts) — 91.66% lines; report adaptation and defaults ([tests](../bike-ui/components/reports/reportDefinitions.test.ts)).
- [x] [bike-ui/lib/activitySourceFiles.ts](../bike-ui/lib/activitySourceFiles.ts) — 100.00% lines; source-file URLs and encoded activity IDs ([tests](../bike-ui/lib/activitySourceFiles.test.ts)).
- [ ] [bike-ui/lib/queries.ts](../bike-ui/lib/queries.ts)
