# Bike Observability Metrics

This dashboard starts with the Prometheus metrics Bike already emits:

- `bike_api_requests_total` and `bike_api_request_duration_seconds_bucket` for RED-style API request rate, errors, and latency.
- `worker_tasks_completed_total`, `worker_tasks_failed_total`, `worker_task_invocations_total`, `worker_task_processing_lag_seconds_bucket`, and `worker_task_duration_seconds_bucket` for background work.
- `bike_provider_api_requests_total`, `bike_provider_api_requests_15_minutes`, `bike_provider_api_requests_daily`, `bike_provider_rate_limit_pauses_total`, `bike_provider_rate_limit_remaining`, `bike_provider_rate_limit_used`, `bike_provider_rate_limit_limit`, and `bike_provider_rate_limit_reset_timestamp_seconds` for Strava provider health.
- `bike_strava_connected_athletes` for the current number of connected Strava athlete accounts.
- `/api/strava/webhook` request metrics and `strava_sync` task metrics for webhook-triggered sync visibility.

## Metrics To Add Next

- `bike_strava_sync_runs_total{status, mode}`: Count sync attempts by outcome and source, such as initial, manual, or webhook.
- `bike_strava_sync_duration_seconds{mode, status}`: Histogram full sync runtime, separate from generic worker task duration.
- `bike_strava_sync_activities_total{mode, outcome}`: Count imported, duplicate, skipped, deleted, and failed activities.
- `bike_strava_webhook_events_total{aspect_type, object_type, outcome}`: Count accepted, ignored, rejected, and failed webhook events without depending only on HTTP status.
- `bike_strava_webhook_event_lag_seconds{aspect_type}`: Time from Strava `event_time` to Bike processing time.
- `bike_provider_api_request_duration_seconds{provider, operation, request_class, status}`: Histogram outbound provider latency so provider slowness is distinct from app request latency.
- `bike_provider_quota_reservations_total{provider, bucket, outcome}`: Count reserved, exhausted, reconciled, and reset quota decisions.
- `bike_provider_quota_wait_seconds{provider, bucket}`: Histogram wait time implied by local or remote rate limiting.
- `bike_strava_token_refreshes_total{status}`: Track token refresh failures before they become sync failures.
- `bike_activity_import_pipeline_stage_duration_seconds{source, stage, status}`: Histogram raw storage, parsing, dedupe, segment matching, and persistence stages.
- `bike_activity_imports_total{source, outcome}`: Count uploads, archive imports, Strava imports, duplicates, and failures.
- `bike_segment_matching_duration_seconds{source}` and `bike_segment_matches_total{source, outcome}`: Track expensive matching work and matching quality.
- `bike_task_queue_depth{type, state}`: Gauge queued, scheduled, retrying, and dead-letter task backlog directly from the task table.
- `bike_integration_events_total{provider, event_type, level}`: Operational count of integration audit events, with metadata kept out of labels.

Good first alerts after these metrics exist:

- Sustained API 5xx rate above 1 percent for 10 minutes.
- API p95 latency above 1 second for 10 minutes.
- Strava 429s or local quota pauses for more than one scrape interval outside expected backfill windows.
- Strava daily read quota remaining below 10 percent.
- `strava_sync` failure rate above 5 percent over 30 minutes.
- Worker p95 processing lag above 5 minutes or task queue depth growing for 15 minutes.
