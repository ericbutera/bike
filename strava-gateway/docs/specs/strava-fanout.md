# Strava gateway delivery contract

Task status is maintained only in [the Bike backlog](../../../docs/TODO.md).
The old story checklist is preserved at `200921f` in Git in Git history. This document describes the delivery contract
and recorded rollout evidence; it does not create additional acceptance gates.

The gateway owns the live Strava subscription and provider credentials. The
Rust receiver accepts signed delivery from the gateway. Current and deferred
work is tracked only in the [Bike TODO](../../../docs/TODO.md).

## Goal

One small service owns the Bike Strava application, OAuth tokens, webhook
subscription, provider quota, and fetched activity payloads. It delivers each
eligible activity change to Bike Rust through durable delivery jobs. The gateway
keeps its database and artifacts separate from Bike application data.

Strava permits one webhook subscription per application. The webhook payload
contains identifiers and change metadata, not the activity and streams. Strava
requires a 200 response within two seconds and retries only a few times. The
HTTP callback therefore commits an inbox row before acknowledging, then a
worker fetches and delivers asynchronously. Sources:
[webhooks](https://developers.strava.com/docs/webhooks/),
[rate limits](https://developers.strava.com/docs/rate-limits/).

## Ownership and data flow

```text
Strava OAuth + webhook -> gateway HTTP -> gateway PostgreSQL inbox
                                      -> gateway worker -> artifact PVC
                                                        -> delivery outbox
                                                        -> Rust receiver
```

The gateway has its own PostgreSQL database and PVC. A single API pod and a
single worker pod are enough initially. PostgreSQL row locks and leases must
keep retries safe if the worker is later replicated. The artifact PVC stores
the fetched provider JSON and a content hash; it is not mounted by Bike sites.
Artifacts remain until all target deliveries reach a terminal state and a
retention window has elapsed. Database backups and PVC backups must cover the
same recovery point, or the worker must be able to refetch a missing artifact.
The worker now detects a missing or corrupt pending artifact, returns its
event to the inbox, and refetches from Strava; this recovery depends on a
usable token and provider quota.

Only the gateway deployment has Strava client credentials and refreshes athlete
tokens. Rust retains its last token pair in its database for the documented
rollback, but its production pods no longer receive Strava client credentials
and its worker skips legacy Strava sync. Site-local connection rows are mirrors
for UI state and must not create/delete subscriptions. Each athlete has
explicit gateway mappings to a local user ID in each Bike site. Mapping is
established from authenticated site-to-gateway OAuth initiation, not inferred
from matching integer user IDs. A target can be disabled independently.

## HTTP contracts

### Strava callback

- `GET /webhooks/strava`: validate `hub.mode`, the exact verification token,
  and a bounded challenge; respond with `{"hub.challenge":"..."}`.
- `POST /webhooks/strava`: cap the request body, validate the configured
  `subscription_id` and supported event fields. The main webhook reference
  does not document a POST
  signature, while Strava's [example](https://developers.strava.com/docs/webhookexample/)
  shows `X-Strava-Signature` without supported signing-key provisioning.
  Set `ALLOW_UNSIGNED_WEBHOOKS=true` without a signing key to accept callbacks
  with or without that header; defer key provisioning and enforcement to
  [LATER12](../../../docs/TODO.md). With unsigned events, the
  subscription ID is only a filter, not authentication: the worker must fetch
  from Strava and must never forward unverified request fields as activity data.
- Insert a canonical event key and raw event into PostgreSQL. A duplicate
  event returns 200. Return non-200 when persistence fails so Strava can retry.
  Do no provider fetch or target delivery in this request.

### Site receiver, version 1

`POST /internal/strava-deliveries` on each API ClusterIP service receives a versioned envelope with
`delivery_id`, `athlete_id`, `site_user_id`, `activity_id`, `event_time`,
`operation`, `content_sha256`, and either fetched activity plus streams or an
explicit deletion/deauthorization. Gateway-to-site requests use a per-site
rotatable HMAC key over timestamp, delivery ID, and exact body. Reject stale
timestamps and wrong signatures before parsing the body. The endpoint is
ClusterIP-only where possible and has a strict body limit.

Each site records `delivery_id` and content hash in a unique receipt table in
the same transaction as its import/delete intent. A duplicate returns 200 with
the prior receipt. Successful `202 Accepted` means the site's durable import
job exists; `200 OK` means it was already accepted or completed. `409` means a
permanent mapping conflict. `429`/`5xx` are retryable. The gateway marks a
delivery complete only after an accepted response. Sites retain their own
activity parsing, analytics, privacy, and derived-state workflows. Existing
numeric and `strava:<activity_id>` correlations are both recognized; new
gateway imports use numeric correlation to match Rust-copied rides.

The receiver must enforce an athlete-to-user mapping stored locally. It must
not trust `site_user_id` alone. Delete and deauthorization must remove or hide
provider-owned data and cancel pending imports without touching independently
uploaded activities. Updates fetch the current full activity and replace or
reprocess the provider-owned record. Events for non-cycling sports do not enter
ride analytics. Privacy changes and out-of-order events require checking the
current provider state before a create/update delivery; a newer delete or
deauthorization must not be undone by an older retry.

## Durable workflow

1. Canonical webhook key includes subscription ID, owner ID, object ID, object
   type, aspect, event time, and sorted updates. The unique inbox constraint
   deduplicates Strava retries without collapsing distinct asynchronous updates.
2. A worker claims pending rows with `FOR UPDATE SKIP LOCKED` and a lease.
   It resolves a gateway athlete connection and current target mappings.
3. For create/update, it refreshes the athlete token if needed, reserves both
   overall and read quota, fetches the current activity and streams once, and
   atomically stores the resulting artifact. Quota pauses set `next_attempt_at`
   rather than sleeping. `429` and rate-limit headers adjust shared counters.
4. It creates one outbox row per enabled target, each with a stable delivery
   ID and artifact hash. Deletes/deauthorizations create outbox rows without a
   provider fetch. Outbox rows are independent and retry with capped backoff,
   jitter, and a dead-letter state; operators can inspect and replay them.
5. Reconciliation periodically compares gateway checkpoints with Strava for
   missed events and backfill. Backfills are resumable and use the same outbox.

Rate-limited sync jobs remain `queued` and persist `waiting_reason` with their
next eligible time. Connection status returns these as `last_sync_wait_reason`
and `last_sync_next_attempt_at` over both HTTP and gRPC; the Rust API passes the
same structured fields through to its connection response.

Each rate-limit retry records a `paused` sync event. When its lease is claimed
again after the eligible time, the same job records `resumed`; this does not
create another activity event until a provider page completes.

No target can claim exactly-once network delivery. Target receipts and source
correlation provide idempotent **effects** under at-least-once delivery.

## Security and operations

- Keep Strava and receiver secrets in Kubernetes Secrets; never log tokens,
  raw activity streams, HMAC bodies, or OAuth codes. Rotate receiver keys.
- Limit ingress to OAuth callback and webhook; `/readyz`, metrics, replay, and
  operator controls stay internal. Use a dedicated Postgres role/database.
- Bound webhook body size and database time so the 200 response meets Strava's
  two-second deadline. Monitor inbox age, outbox age per target, dead letters,
  refresh failures, provider 429s, PVC use, and signature failures.
- Size initial pods at 100m/128Mi for HTTP and 100m/256Mi for worker, then
  measure. The artifact PVC starts at 5Gi and can grow; no streaming broker.
- Never treat the old Rust and new gateway rate-limit counters as independent
  while both can call the same Strava app. Cut over API ownership deliberately.

## Verification and rollout evidence

The owning gateway tests cover provider failures, persistent inbox/delivery jobs,
quota pauses, leased retries, sync checkpoints, and artifact repair. Integration
tests use an explicitly selected disposable PostgreSQL schema. Provider fakes
are recorded separately from live callbacks and production verification.

The gateway and Rust receiver were deployed and verified in 2026-09-29's initial
rollout. Subsequent 2026-10-02 records cover provider exchange/refresh, the
original-ride fixture, and signature-mode callback recovery. See the
[fixture provenance](../../internal/provider/testdata/README.md),
[production failure runbook](../../../docs/production-failures.md), and
[active backlog](../../../docs/TODO.md) for exact scope and remaining work.
