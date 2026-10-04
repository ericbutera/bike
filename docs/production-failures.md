# Production failures and regression fixtures

This is the STRAVA15 runbook. At roughly ten new activities per day, one
unresolved import or sync failure is actionable. Reuse PostgreSQL, protected
activity/artifact storage, JSON logs in Loki, Prometheus, Alertmanager, and the
existing ntfy destination.

## What remains available after failure

| Failure                                               | Retained evidence                                                                                                                                           | Recovery                                                                                                         |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Rejected inbound Strava callback before enqueue       | Structured gateway warning/error with rejection reason, HTTP status, trace IDs when available, and safe header/query presence flags; HTTP failure counter   | Correct signature/configuration/payload/persistence cause, then run incremental sync for each affected site link |
| Invalid Strava activity/list/streams response         | Dead webhook or sync job, last error, diagnostic file containing stage and the exact response bytes encoded as base64                                       | Fix the adapter/workflow, then replay the affected job                                                           |
| Exhausted retries or terminal site delivery rejection | Dead job, last error, attempts, athlete/activity IDs, target, trace context, and original delivery artifact where applicable                                | Correct the cause, then replay only the failed target                                                            |
| Failed FIT/GPX/TCX or downstream import processing    | Existing `activity_imports` row with format, original filename, storage path, stage, error, attempts, and processing events; original file stays in uploads | Fix the parser/workflow, then use the existing import reprocessing operation                                     |
| Temporary provider/network failure                    | Queued retry with last error and next attempt, structured warning, pending-job age                                                                          | Let normal retries recover; investigate the age alert if work remains stuck                                      |
| Scheduled provider quota wait                         | Queued job with quota reason and reset/next-attempt time; informational log                                                                                 | Let the existing quota scheduler resume it                                                                       |

Unsupported provider payloads stop after their first capture instead of
redownloading an unparseable response twenty times. Expected missing streams,
deleted/private activities, and non-cycling activities keep their existing
handling. Diagnostic files contain activity response bodies, never OAuth token
responses or authorization headers. Files remain protected with mode `0600`
and content-addressed SHA256 paths.

Worker error logs include queue, job ID, attempt, athlete/activity ID, target or
page when applicable, disposition, trace IDs when available, and artifact
path/hash and parsing stage for captured provider failures. IDs belong in logs
and durable records; Prometheus labels stay bounded to queue/site/stage.

## Alerts and notifications

[The infrastructure rules](https://github.com/ericbutera/pulumi-iac) notify
through the existing Alertmanager -> ntfy route to the configured ntfy topic. Warning alerts repeat every
four hours, critical alerts hourly; recovery sends a resolved notification.
The configured URL uses ntfy's [built-in Alertmanager message
template](https://docs.ntfy.sh/publish/#message-templating). Watchdog,
InfoInhibitor, and informational quota pauses stay out of this notification
channel. InfoInhibitor retains its namespace-scoped inhibition of info alerts.

- `BikeStravaDeadLetters`: any retained dead job for two minutes, including
  after a worker restart; clears after replay removes the dead state.
- `BikeStravaWebhookRejected`: one inbound callback returning 4xx/5xx during
  the last hour, including a first error series without a zero baseline, for
  one minute. Covers signature, JSON, subscription, verification, method,
  body-limit, and inbox-persistence failures before a job exists. Logs identify
  the reason; callbacks accepted or deduplicated also produce an activity-ID
  log. Healthy probes remain quiet. Recovery clears the alert once the last
  failure leaves the one-hour window.
- `BikeActivityImportsFailed`: any failed Bike import for two
  minutes; clears when its persisted status changes after reprocessing.
- `BikeImportFailureMetricsUnavailable`: a healthy API scrape lacks a usable
  database-backed import count for five minutes. A scrape/query error must not
  masquerade as zero failures.
- `BikeStravaGatewayOldQueueItems`: any eligible item over ten minutes old for
  two minutes; future scheduled sync quota waits are excluded.
- `BikeStravaWorkerLeaseExpired`: a lease remains expired for five minutes.
- API server errors, worker task failures, and map render failures use a single
  failure during the last hour rather than high request-rate thresholds.
- Backlog thresholds reflect ten activities/day and up to three deliveries
  per activity. Daily quota exhaustion and repeated provider 429s remain
  distinct warning signals.
- `BikeAlertDeliveryFailed`: Alertmanager's webhook failed. This remains
  visible in Prometheus even if the same notification channel is unavailable.
  Existing target-down, backup, and storage-capacity alerts remain enabled.

Alertmanager delivery counters prove server-side webhook attempts and errors;
they do not prove that a particular phone has subscribed to the ntfy topic.
The always-firing `Watchdog` and `InfoInhibitor` rules are routing helpers, not
pages: both must select the `null` receiver. If either arrives through ntfy,
check Alertmanager's route order and fix that route; do not silence the rule.

## Alert runbooks

Use the alert's namespace, queue, bucket, and stage labels to pick the owning
service. Keep credentials and activity payloads out of logs and incident notes.

| Alert                                                                                     | First response at 3AM                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Recovery signal                                                                                                                                                     |
| ----------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <a id="bikestravawebhookrejected"></a>`BikeStravaWebhookRejected`                         | Open the Strava gateway logs panel with the alert time range. Find `Strava webhook rejected` and its `reason`, status, signature/query presence flags, and trace IDs. `invalid_signature` requires checking unsigned mode/key configuration; `invalid_json`, `invalid_event`, or `subscription_mismatch` require checking the provider contract/subscription. For `inbox_unavailable`, check PostgreSQL and gateway readiness. Verification failures can come from a GET probe without Strava's challenge parameters. The callback may have no durable job: fix the cause, then use incremental sync for each affected site instead of dead-job replay. Bodies, tokens, and signature contents are intentionally absent from logs. | Subsequent real callbacks return 200 and log `accepted`/`duplicate`; the activity is imported. The alert resolves after one hour without another rejected callback. |
| `Watchdog`                                                                                | This is the monitoring heartbeat and should always fire in Prometheus. If it reaches ntfy, inspect Alertmanager route order; its receiver must be `null`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | It remains visible in Prometheus and sends no ntfy message.                                                                                                         |
| `InfoInhibitor`                                                                           | This helper groups namespace info alerts and should route to `null`. If it reaches ntfy, inspect the null route and namespace-scoped inhibition.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | It is firing only while relevant info alerts exist and sends no ntfy message.                                                                                       |
| <a id="bikebackupstale"></a>`BikeBackupStale`                                             | Check `kubectl -n pg get cronjob,jobs`, the latest Job logs, and both host `last-success.json` files. Follow the [backup runbook](../../pulumi-iac/docs/Bike-Backup-Runbook.md#failure-and-visibility); repair a missing disk mount before rerunning the job.                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | A successful Job validates and publishes both disk snapshots.                                                                                                       |
| <a id="bikescrapetargetdown"></a>`BikeScrapeTargetDown`                                   | Check pods, Services, endpoints, ServiceMonitor, and events in the labeled namespace. A Strava worker target is absent by design when its replica count is zero.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | The expected Prometheus `up` series returns to `1`.                                                                                                                 |
| <a id="bikesiteapifailures"></a>`BikeSiteApiFailures`                                     | Check `kubectl -n <namespace> logs deploy/api --since=1h`; match route/status to its trace and fix the failing request or dependency. Roll back a rollout causing broad 5xxs.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | The error leaves the one-hour window.                                                                                                                               |
| <a id="bikesiteapip95latencyhigh"></a>`BikeSiteApiP95LatencyHigh`                         | Use the API dashboard to identify slow routes and inspect slow traces for database, gateway, or map-render waits. Verify the slow requests before changing capacity for this low-volume app.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | p95 stays below two seconds for ten minutes.                                                                                                                        |
| <a id="bikeworkerfailures"></a>`BikeWorkerFailures`                                       | Check worker logs and the failed task's type, last error, attempts, and trace. For imports, inspect processing events and the retained original file. Retry only after fixing the cause.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | The task/import completes; transient worker failure ages out of the one-hour window.                                                                                |
| <a id="bikeworkerqueuebacklog"></a>`BikeWorkerQueueBacklog`                               | Check worker readiness, ready count, oldest age, and PostgreSQL health. Restore the worker or DB connection and let the queue drain; check for a paused dependency before replaying anything.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | Ready count stays at or below ten for fifteen minutes.                                                                                                              |
| <a id="bikestravagatewayqueuebacklog"></a>`BikeStravaGatewayQueueBacklog`                 | Check which gateway queue is growing, Strava worker health/logs, and quota pauses. Restore the worker or dependency; don't increase concurrency to push through provider quota.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Queue count is falling and returns below its threshold.                                                                                                             |
| <a id="bikestravagatewayoldqueueitems"></a>`BikeStravaGatewayOldQueueItems`               | Inspect the oldest eligible item and last error in the gateway dashboard. Quota-scheduled items are excluded until eligible; fix worker/dependency issues and watch eligible age.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Oldest eligible item is below ten minutes.                                                                                                                          |
| <a id="bikestravagatewayquotalow"></a>`BikeStravaGatewayQuotaLow`                         | Check the labeled bucket's remaining requests and reset time. Let normal work wait for quota; do not increase request concurrency.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | Bucket recovers above five percent or resets.                                                                                                                       |
| <a id="bikestravagatewayprovider429sustained"></a>`BikeStravaGatewayProvider429Sustained` | Check 429s by bucket, reset time, and whether the gateway honors `Retry-After`. Leave work queued until the window resets; fix only a request path exceeding quota.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Provider 429s stop.                                                                                                                                                 |
| <a id="bikestravagatewayquotapaused"></a>`BikeStravaGatewayQuotaPaused`                   | This is informational: check reset time and queued work, then let the normal scheduler resume it. Escalate only if eligible work also exceeds the age limit.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | Scheduler resumes queued work after reset.                                                                                                                          |
| <a id="bikestravagatewaydailyquotaexhausted"></a>`BikeStravaGatewayDailyQuotaExhausted`   | Check the bucket's UTC reset time and let the scheduler hold jobs until then. Do not manually increase throughput or replay paused work.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | The daily bucket resets and work resumes.                                                                                                                           |
| <a id="bikestravadeadletters"></a>`BikeStravaDeadLetters`                                 | Run `/app/admin failures` in `strava-worker`; inspect the retained error, attempts, logs, trace, and protected artifact. Add a fixture, fix the owning seam, and replay only the failed event/delivery/sync.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | The job leaves the dead-letter queue and its delivery succeeds.                                                                                                     |
| `BikeActivityImportsFailed`                                                               | Use the detailed [failed-import procedure](#bikeactivityimportsfailed): inspect the row, processing events, source file, and linked activity; fix the parser/workflow and retry only that import.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Genuine failures are resolved/reprocessed; the namespace's stored failed count reaches zero.                                                                        |
| <a id="bikeimportfailuremetricsunavailable"></a>`BikeImportFailureMetricsUnavailable`     | Check API logs and its protected DB connection/permissions; verify the `activity_imports` query works and deploy a matching API build if the gauge is missing. Never interpret missing as zero.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | API exposes a finite, nonnegative failure count.                                                                                                                    |
| <a id="bikestravaworkerleaseexpired"></a>`BikeStravaWorkerLeaseExpired`                   | Check worker health/logs and PostgreSQL access. Lease reclaim is automatic; restore the worker/DB and avoid editing a lease while a worker may own it.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Stuck-job metric returns to zero.                                                                                                                                   |
| <a id="bikealertdeliveryfailed"></a>`BikeAlertDeliveryFailed`                             | Check `kubectl -n observability logs statefulset/alertmanager-kube-prom-stack-kube-prome-alertmanager --since=30m`, the config reload, and outbound access to the configured ntfy endpoint. Keep Prometheus visible while ntfy is down; use the on-demand delivery check only if needed.                                                                                                                                                                                                                                                                                                                                                                                                                                     | Webhook failures stop and a normal notification is delivered.                                                                                                       |
| <a id="bikemapcacheneartcapacity"></a>`BikeMapCacheNearCapacity`                          | Check renderer logs and cache pruning. This is app directory bytes versus PVC request, not physical free disk; prune only disposable cache entries through the normal path.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | Usage falls below ninety percent.                                                                                                                                   |
| <a id="bikestravaartifactsnearcapacity"></a>`BikeStravaArtifactsNearCapacity`             | Check artifact growth and completed-delivery retention. Preserve artifacts for active retries/dead letters; let configured retention remove completed deliveries.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Usage falls below ninety percent.                                                                                                                                   |
| <a id="bikemaprenderfailures"></a>`BikeMapRenderFailures`                                 | Use the labeled stage and renderer dashboard; check renderer logs, cache PVC, browser availability, and one affected map. Fix the failing stage and verify a normal render.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | Failure leaves the one-hour window.                                                                                                                                 |
| <a id="biketelemetrycollectorunavailable"></a>`BikeTelemetryCollectorUnavailable`         | Check collector pods, Service/endpoints, and logs in `observability`; restore the collector target and verify spans reach Tempo.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | Collector target is healthy and spans flow again.                                                                                                                   |
| <a id="biketelemetryexporterqueuehigh"></a>`BikeTelemetryExporterQueueHigh`               | Check collector queue size/capacity, Tempo health, and retry/backoff logs. Restore Tempo connectivity/capacity and let queued spans drain.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | Queue remains below 75 percent for ten minutes.                                                                                                                     |
| <a id="biketelemetryexportfailures"></a>`BikeTelemetryExportFailures`                     | Check collector failed-export/retry counters and logs, then verify Tempo service/network access. Restore the path and confirm failures stop and the queue drains.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Export failures stop.                                                                                                                                               |

## BikeActivityImportsFailed

Use the alert's namespace and count to inspect failed rows through that site's
protected database connection using the deployment's `DATABASE_URL`. Start
with this read-only query:

```sql
SELECT id, user_id, source, format, status, processing_stage,
       processing_attempts, processing_error, storage_path, activity_id
FROM activity_imports
WHERE status = 'failed'
ORDER BY last_processing_event_at DESC NULLS LAST
LIMIT 50;
```

For one import, inspect its durable processing history:

```sql
SELECT event_type, created_at, message, payload
FROM integration_events
WHERE provider = 'activity_processing'
  AND payload->>'import_id' = '<import-id>'
ORDER BY created_at;
```

Compare the stage/error with the original file at `storage_path` on that site's
uploads volume. Preserve a parse/format failure, add a small fixture from the
file (sanitize personal data), and deploy the parser/workflow fix before retrying.
Choose the recovery operation for the failed import:

- When an activity is linked: an administrator can
  `POST /api/admin/activity-imports/reprocess-activity` with JSON
  `{"activity_id": <activity-id>}`. Track the returned task ID.
- When parsing failed before an activity was created: find the matching
  `process_activity_import` task using the query below. Rust has no per-import
  retry endpoint; uploading the same bytes returns the existing import. After
  the fix is deployed, set only that failed import back to `processing`, then
  use the admin task's Rerun action or `POST /api/admin/tasks/<task-id>/rerun`.
  The worker skips imports whose status is still `failed`. If there is no
  matching task, preserve the failure and escalate with its original file;
  do not replay an entire archive or sync to recover one import.

```sql
SELECT id, status, error, payload
FROM background_tasks
WHERE task_type = 'process_activity_import'
  AND COALESCE(payload->'data', payload)->>'import_id' = '<import-id>'
ORDER BY created_at DESC
LIMIT 5;
```

For that Rust task rerun, verify its payload's user/import IDs and that no
matching task is already pending/processing. Then prepare only that import:

```sql
UPDATE activity_imports AS i
SET status = 'processing', processing_stage = 'raw_stored', updated_at = now()
WHERE i.id = <import-id> AND i.status = 'failed'
  AND NOT EXISTS (
    SELECT 1 FROM background_tasks AS t
    WHERE t.task_type = 'process_activity_import'
      AND t.status IN ('pending', 'processing')
      AND COALESCE(t.payload->'data', t.payload)->>'import_id' = i.id::text
  )
RETURNING i.id, i.status;
```

Require one returned row, rerun the matching task once, and follow its new task
ID to completion. If the rerun request fails to enqueue, restore this import's
status to `failed` while preserving its error. A zero count during processing
is not proof of recovery: confirm the task succeeds and the activity is present.
Do not mark a missing-activity import processed or delete its file/history.

The known false failures from the old Rust resume bug have the exact error
`Activity import <id> has no linked activity to resume`, `source =
'archive_url_import'`, and `processing_stage = 'complete'`. After the Rust fix is
deployed, verify the related archive job completed with zero failed entries and
confirm no row in `activities` references each import. Then retire only those
orphan rows from the alert count while retaining the diagnostic and events:

```sql
BEGIN;
SET LOCAL lock_timeout = '5s';
CREATE TEMP TABLE legacy_orphan_imports ON COMMIT DROP AS
SELECT i.id, i.user_id, i.processing_error, i.processing_attempts
FROM activity_imports AS i
WHERE i.status = 'failed'
  AND i.source = 'archive_url_import'
  AND i.processing_stage = 'complete'
  AND i.activity_id IS NULL
  AND i.processing_error = 'Activity import ' || i.id::text || ' has no linked activity to resume'
  AND NOT EXISTS (
    SELECT 1 FROM activities AS a WHERE a.activity_import_id = i.id
  )
  AND EXISTS (
    SELECT 1 FROM activity_archive_import_jobs AS j
    WHERE j.user_id = i.user_id AND j.status = 'succeeded' AND j.failed_count = 0
      AND i.created_at BETWEEN j.started_at AND j.finished_at
  );
SELECT * FROM legacy_orphan_imports;
-- Review these rows against the successful archive job and expected count.
-- If anything differs, ROLLBACK instead of running the statements below.
UPDATE activity_imports AS i
SET status = 'canceled', updated_at = now(), last_processing_event_at = now()
FROM legacy_orphan_imports AS legacy
WHERE i.id = legacy.id;
INSERT INTO integration_events
  (user_id, provider, event_type, level, message, payload, created_at)
SELECT user_id, 'activity_processing', 'import_canceled', 'info',
       'Retired verified legacy orphan failure; original evidence retained',
       jsonb_build_object('import_id', id,
                          'reason', 'legacy_missing_activity_resume',
                          'original_error', processing_error,
                          'previous_processing_attempts', processing_attempts),
       now()
FROM legacy_orphan_imports;
COMMIT;
```

This preserves the error text, original file, and every event. Do not run the
cleanup before the fixed image is live. Genuine failed imports remain `failed`
and continue to alert. Confirm the namespace metric reaches zero and the alert
resolves.

## Investigate, reproduce, fix, replay

List unresolved gateway failures and correlate their IDs with worker logs:

```sh
kubectl -n bike-services exec deploy/strava-worker -- /app/admin failures
kubectl -n bike-services logs deploy/strava-worker --since=1h
```

For a captured payload, copy its SHA256 from the last error or structured log.
The worker has the artifact volume mounted; its image does not require a shell
or `tar` to export a capture:

```sh
umask 077
kubectl -n bike-services exec deploy/strava-worker -- /app/admin artifact <sha256> > /tmp/strava-capture.json
mise exec -- ruby -rjson -rbase64 -e 'print Base64.strict_decode64(JSON.parse(File.read(ARGV[0])).fetch("body_base64"))' /tmp/strava-capture.json > /tmp/strava-response.json
```

Review and sanitize athlete IDs, names, GPS coordinates, and any other personal
data before checking a derived fixture into Git. Keep the protected original
capture for investigation. Give the fixture the supported provider response
shape, or use an original ride in `bike-rs/data` for downstream parsing tests.
Add the smallest regression at the provider interface, parser, application
workflow, or database boundary. Prefer a fake provider/receiver and the owning
native test suite. Browser login, live SSO, and live Strava are unnecessary for
this regression. Playwright/k6 remain platform synthetic tools for activity
list/detail, segment list/detail, and race viewer.

After the regression passes and the fix is deployed:

```sh
kubectl -n bike-services exec deploy/strava-worker -- /app/admin replay event <id>
# For a failed site target, use its delivery ID:
kubectl -n bike-services exec deploy/strava-worker -- /app/admin replay delivery <id>
# For a failed activity-list sync, use its sync ID:
kubectl -n bike-services exec deploy/strava-worker -- /app/admin replay sync <id>
```

Replay only the corresponding command. The existing operation accepts dead
jobs only and preserves successful deliveries. Confirm the job/import leaves
the failed state, its activity becomes available, and the persistent alert
resolves. Failed captures have no new automatic deletion policy.

## Verification and deployment

Gateway tests use fake providers and isolated database fixtures to exercise
durable failure, protected capture, replay, and valid activity delivery. Rust's
native lifecycle checks verify failed imports and recovery independently.

From the infrastructure repository, run:

```sh
mise run check:bike:alerting
mise run deploy:bike:alerting
```

Validation runs pinned Prometheus rule checks and fixtures, Alertmanager
configuration validation, and receiver-routing checks. It sends no notifications.
The deployment task applies the owned alert configuration and rules to the
existing monitoring release. Application and gateway images deploy through
their component release workflows.

Rejected inbound callbacks produce bounded reasons, HTTP status, trace IDs, and
header/query presence flags. Tests cover invalid JSON, subscription mismatch,
signature rejection, persistence failure, and omission of secrets and request
bodies. The rejection alert covers the boundary before a durable job exists;
retained-job alerts cover failures after persistence.

The [production verification record](production-verification.md) contains the
latest deployment, dashboard, scrape-target, rule-health, and actual firing and
recovery notification results. Historical recovery details remain in the
[infrastructure repository](https://github.com/ericbutera/pulumi-iac).
