# Admin Operations Specification

Admin operations give Bike a way to inspect system state, re-run derived processing, and diagnose integrations without hand-editing database rows.

## Product Intent

Admin tools should be boring, explicit, and recoverable. They exist to repair or inspect system state while preserving user data and respecting normal processing contracts.

## Admin Surfaces

Admin navigation exposes operational pages for:

- analytics and manual tasks;
- users;
- feature flags;
- metrics;
- integration events;
- background tasks.

Admin pages require an authenticated admin user. Non-admin users should not be able to reach admin-only operations through either UI or API.

## Metrics

Bike exposes app metrics in addition to shared system metrics. Metrics should be suitable for dashboards and alerting without leaking sensitive user data.

The app metrics page should distinguish Bike-specific stats from framework or infrastructure metrics.

## Backfills And Manual Tasks

Admin manual tasks can enqueue work such as:

- analytics backfill;
- XC training backfill for a user;
- user segment regeneration;
- specific segment effort regeneration;
- user activity import reprocessing;
- duplicate activity cleanup;
- archive import for a user.

Manual tasks should queue background work where possible. They should return task ids, queued status, or clear summaries rather than doing expensive work in the admin request path.

## Integration Event History

Integration events provide an audit trail for provider behavior, especially Strava. Events should include provider, event type, level, user when known, message, metadata, and timestamp.

The user-facing account page may show a filtered connection history. Admin pages may show broader integration history for debugging.

## Feature Flags

Feature flags are operational switches. They should not become hidden product requirements. If a feature flag changes intended behavior, the relevant spec should describe both the enabled behavior and the fallback.

## Code Anchors

- Admin API: `api/src/controllers/admin.rs`
- Integration event API: `api/src/controllers/integration_events.rs`
- Metrics: `api/src/metrics.rs`
- Admin task UI: `ui-next/components/admin/AdminTaskTools.tsx`
- Admin metrics UI: `ui-next/components/admin/BikeMetricsSection.tsx`
- Admin navigation: `ui-next/components/admin/Nav.tsx`

## Open Gaps

- Keep admin task responses consistent as more background jobs are added.
- Add focused tests for admin authorization on high-impact operations.
- Keep integration event metadata structured enough for filtering without exposing token or secret material.
