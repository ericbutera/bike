# Bike API and data-access review

The [source-route inventory](api-route-audit.json) retains a historical source
review. It is not an automated route or authorization check. Use the owning
native HTTP tests (`mise run test:integration`) for route behavior and review
OpenAPI changes alongside controller registrations. `mise run contracts:check`
compares distributed asset copies with their canonical owners.

The owning controller adapts HTTP; application services delegate queries to
entity/model modules. Bounds belong in SQL before materialization, while
calculations must preserve their complete scoped input.

## Query review

This table retains the Rust source-review baseline from 2026-10-01. It describes
query intent, not a fresh executed-SQL measurement. Paths are relative to
`/api` unless marked otherwise. The [backlog](TODO.md) records verification
still needed for import, matching, training, and recovery changes.

| API family                                                                       | Rust data-access baseline                                                                                                                                               |
| -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `GET /activities`                                                                | Owner-filtered count and ordered SQL page; achievements restricted to page IDs. Route previews require the selected activities' derived data.                           |
| `/activities/{id}`, update/delete/regenerate/source-file                         | Individual activity and ownership lookup; detail relations restricted to its activity/segment/import IDs.                                                               |
| `GET /admin/activities` and activity trace                                       | Count and SQL page; selected metadata columns and page-linked imports. Trace selects one activity/import.                                                               |
| `GET /activity-imports`                                                          | Latest 25 **manual-upload** imports for the owner; batch load linked activities, including the activity's import backlink.                                              |
| Import upload/retry/archive submission                                           | Owner/source/checksum or ID lookups, artifact writes and durable work.                                                                                                  |
| `/activity-imports/processing-state`                                             | One owner-scoped processing lock; a stale manual-upload lock is released.                                                                                               |
| `/activity-imports/processing-graph`                                             | Static graph, no activity/result-set query.                                                                                                                             |
| User/admin import trace                                                          | Owner/provider/import JSON predicate **before** newest-first limit 100.                                                                                                 |
| Archive-job list/detail                                                          | Latest 10 owner-scoped jobs; owner + job ID for detail.                                                                                                                 |
| `GET /segments`                                                                  | All owner segment **metadata**, scoped persisted summaries; no route or effort payloads. Ridden segments sort ahead of unused segments.                                 |
| `GET /segments/{id}`                                                             | One owned segment plus persisted summaries; no effort activity routes.                                                                                                  |
| Segment comparison                                                               | One owned segment, its efforts and only referenced activity routes/rider names.                                                                                         |
| Segment yearly bests                                                             | Owner/segment/user efforts and activity ID/title/start time; no activity route JSON.                                                                                    |
| Segment effort analysis                                                          | All eligible owner/segment efforts and referenced routes, then calculate splits/theoretical bests. `effort_limit` limits displayed results, not the calculation corpus. |
| Segment import/from-activity/update/delete                                       | Owner/ID lookups; duplicate candidates restricted by owner and five-meter distance bucket before route-key comparison.                                                  |
| `/fitness` and analytics backfill                                                | Cached rows constrained by owner/day range; rebuild reads owner rides from dirty day using training-load columns. Backfill enumerates distinct user IDs.                |
| `/training/xc-progress`                                                          | Summary fields for analysed owner rides and races; scoped analyses/preferences/latest fitness.                                                                          |
| `/training/dh-progress`                                                          | Owned DH segments, owner efforts, referenced cycling activity summaries.                                                                                                |
| `/training/reports`                                                              | Owner, ride sport, requested dates and minimums in SQL. Explicit comparison selections are ID-scoped independently of candidate dates.                                  |
| Report definitions                                                               | Static definitions.                                                                                                                                                     |
| `/integration-events/strava`                                                     | Owner/provider and newest-first limit 25.                                                                                                                               |
| `/admin/integration-events`                                                      | Optional provider/user/activity/import predicates before limit (default 100, clamp 1–200); complete event fields.                                                       |
| Strava connect/connection/disconnect/sync/callback/webhook and internal delivery | Individual owner/athlete/state/receipt/checkpoint/watermark lookups; delivery mutations and queueing.                                                                   |
| Preferences, current user, refresh/logout, OAuth                                 | Owner/token/provider/state lookups; preference/session mutations.                                                                                                       |
| Admin users and tasks                                                            | SQL search/status/date predicates, count and pagination; ID-scoped detail/update/cancel/rerun.                                                                          |
| Feature flags                                                                    | Entire small flag set, or flag-key update.                                                                                                                              |
| Metrics/health/root                                                              | Database aggregates or process counters/static configuration.                                                                                                           |
| Admin bulk cleanup/reprocess/segment/analytics/XC operations                     | Explicit owner-wide workflows and durable tasks, not interactive list pages.                                                                                            |

## Evidence and limits

DATA02's 2026-10-01 activity-list fixture review recorded three Rust queries
for one owner-scoped page using fixture `408ced4a24db0e7c33725ee9fb526c0c`.
Historical detailed measurements remain in Git through `043445f`.
This result does not establish production latency, worker memory, or every route's
SQL behavior. Reuse the owning test and one relevant fixture when a query changes.
