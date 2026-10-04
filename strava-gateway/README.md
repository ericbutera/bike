# Bike Strava gateway

This service lives in the Bike repository beside the map renderer.
It owns Bike's Strava OAuth callback, webhook inbox, encrypted provider
credentials, activity fetching, artifact storage, and signed delivery to Bike
Rust.

The PostgreSQL inbox acknowledges webhooks after a durable commit. A worker
fetches activity detail and streams, stores a content-addressed artifact on a
persistent volume, and sends the artifact to the Rust receiver. If a pending
delivery's artifact is missing or corrupt, the worker queues the event for a
fresh provider fetch and repairs the artifact.

## Processes

- `/app/migrate` applies append-only Goose migrations before serving.
- `/app/gateway` serves `GET/POST /webhooks/strava-gateway`,
  `GET /oauth/gateway/callback`, compatibility callback paths, signed internal
  site commands under `/v1`, and `/healthz` and `/readyz`.
- `/app/worker` leases inbox, delivery, and sync jobs. Only one worker replica
  is supported during the initial rollout.
- `/app/admin status` prints inbox, outbox, and sync job counts by status.
  `/app/admin replay <event|delivery|sync> <id>` requeues a dead job by ID.
  Run it inside the worker pod; neither command has a public HTTP route.
- `/app/import-rust` can transfer encrypted tokens and the Rust athlete-to-user
  mapping into the gateway transactionally. It never prints tokens or replaces
  an existing gateway token by default.
- `/app/export-rust` checks or restores a matching Rust connection and token
  pair for rollback. Stop gateway workers before applying a rollback export.

The webhook body limit is 32 KiB and its database deadline is 1.5 seconds. It
returns 503 when the inbox cannot commit, allowing a Strava retry. OAuth and
provider calls never run on the webhook request path.

## Configuration

Both long-running processes need `DATABASE_URL`, `TOKEN_ENCRYPTION_KEY`
(base64 of 32 random bytes), `STRAVA_CLIENT_ID`, `STRAVA_CLIENT_SECRET`,
`STRAVA_WEBHOOK_VERIFY_TOKEN`, and `TARGET_RUST_SECRET`. The HTTP process also
needs `STRAVA_SUBSCRIPTION_ID`, `STRAVA_CALLBACK_URL`, and `RUST_ACCOUNT_URL`.
The worker needs `ARTIFACTS_DIR` and `TARGET_RUST_URL`.

The gateway and worker accept positive integer overrides for Strava quota
buckets: `STRAVA_QUOTA_OVERALL_15M_LIMIT`, `STRAVA_QUOTA_OVERALL_DAILY_LIMIT`,
`STRAVA_QUOTA_READ_15M_LIMIT`, and `STRAVA_QUOTA_READ_DAILY_LIMIT`. Unset or
empty values keep the defaults of 200/2,000 overall requests and 100/1,000 read
requests per 15-minute/UTC-day window. An invalid, zero, or negative override
stops startup with a validation error.

Strava's webhook example includes a signature, but no supported way to obtain
its signing key is documented. Set `ALLOW_UNSIGNED_WEBHOOKS=true` and leave
`STRAVA_WEBHOOK_SIGNING_SECRET` unset. In this explicit unsigned mode,
callbacks are accepted whether or not they carry `X-Strava-Signature`; the
subscription ID, payload limits, validation, and durable persistence still
apply. A configured signing key validates any supplied signature. Never expose
the signed `/v1` routes through public ingress.

The production stack lives in `../../pulumi-iac/bike-services`. Its public
ingress exposes only the OAuth callback and webhook on the existing Bike
callback host. The worker mounts a protected 5 Gi PVC at `/data/artifacts`.
Production runs on `linux/amd64`; manual image builds from Apple Silicon must
set `--platform linux/amd64` before publishing an immutable commit tag.

## Verification

Use the [production failure runbook](../docs/production-failures.md) for
alerting, protected failure captures, artifact export, and selective dead-job
replay.

Run `mise run test` and `mise run build` from this directory. For PostgreSQL
integration checks, run `docker compose up -d postgres` and set
`TEST_DATABASE_URL` to its published port before running the integration task.
The checks apply migrations twice and cover inbox, outbox, quota, sync pages,
reconciliation, and dead-letter replay.
