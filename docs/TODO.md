# Strava integration TODO

Goal: one job with named steps, clear inputs and outputs, and visible progress
from receipt to finished results.

Approach: Temporal with Rust workers. Use the shared Nibelheim service in
production and Docker Compose locally. Reuse Bike's existing domain code.

## Setup

- [x] Document the upsert/delete pilot workflows, step inputs/outputs, and completion rule in the [Strava spec](specs/strava-worker-workflows.md).
- [x] Document the local Temporal setup and shared production service target.
- [x] Run Temporal and its UI alongside Bike in Docker with PostgreSQL storage.
- [x] Connect the Rust Temporal worker using a configurable service address and the `bike` namespace.
- [ ] Measure memory use and disk growth before choosing production limits.

## Operations

- [x] Implement local Temporal upsert/delete workflows with named activities.
- [ ] Activity create webhook: dedicated create workflow.
- [ ] Activity update webhook: dedicated update workflow for title, type, and privacy.
- [ ] Activity upsert: dedicated workflow used by sync and backfill.
- [ ] Activity delete webhook: dedicated delete workflow.
- [ ] Incremental sync: dedicated workflow using a saved offset.
- [ ] Backfill: dedicated workflow with explicit start and end dates. Keep automatic initial sync limited to 15 days.
- [ ] Disconnect: dedicated workflow for manual disconnect and Strava deauthorization.
- [ ] Keep the scheduled connection scan separate from each connection's sync.
- [ ] Define how reconciliation repairs missed updates, deletes, and older uploads.
- [ ] Define ordering for webhooks that share the same event timestamp.

## Steps and job history

- [ ] Give each step its own named processor, typed input, typed output, and retry policy.
- [ ] Define each workflow's steps and dependencies in one place.
- [ ] Keep webhook receipt durable before acknowledging it. Make the handoff to Temporal retryable without starting duplicate jobs.
- [ ] Give each job one identity from webhook or sync request through all child work.
- [ ] Map Temporal failure/cancellation back to Bike's receipt status and account history.
- [ ] Wait for child operations and required derived outputs before reporting success or advancing the sync offset.
- [ ] Make cancellation stop further work and reach child jobs.
- [ ] Provide one job view showing the graph, current step, inputs, outputs, attempts, errors, and final result.
- [ ] Keep raw JSON collapsed. Link every step to its processor implementation.

## Verification

- [x] Verify the pilot with real Temporal, isolated PostgreSQL, fake Strava, and real derived processors: create/update, unchanged data, delete, stale events, retries, cancellation, and failed derived work.
- [ ] Resolve Temporal cold-start warning/error logs before treating startup checks as clean.
- [ ] Test every operation's happy path with fake Strava responses.
- [ ] Test duplicate and out-of-order webhooks without duplicate or stale writes.
- [ ] Test multiple listing pages, offset boundaries, and historical backfill.
- [ ] Test rate-limit pauses and provider failures without losing progress.
- [ ] Test worker restart before and after database writes without duplicate effects.
- [ ] Test cancellation and failed children using the real workflow runtime.
- [ ] Verify the job view explains a successful job and a failed job from start to finish.

## Cutover

- [x] Prove the local upsert and delete pilot.
- [ ] Move webhook intake and the remaining operations to Temporal.
- [ ] Drain existing jobs before removing the old Strava orchestration and generic `strava_workflow` processor.
- [ ] Verify production create, update, delete, sync, backfill, and disconnect results before marking this checklist complete.
