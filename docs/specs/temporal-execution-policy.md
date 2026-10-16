# Temporal execution policy

- [Shared platform policy](../../bike-rs/bike-core/src/platform/temporal.rs) owns
  named timeout constants and native retry-policy factories. Bike and Strava
  workflow options use these factories; workflow steps do not build policies.
- Keep retry settings explicit: initial interval, backoff coefficient, maximum
  interval, and maximum attempts. Attempts include the initial execution.
- Preserve current execution budgets:

| Activity role        | Execution timeout     | Heartbeat timeout | Retry factory             | Total attempts | Initial interval | Backoff | Maximum interval |
| -------------------- | --------------------- | ----------------- | ------------------------- | -------------- | ---------------- | ------- | ---------------- |
| Bike domain effect   | 2 minutes per attempt | None by default   | `activity_retry_policy()` | 12             | 1 second         | 2×      | 1 minute         |
| Strava domain effect | 2 minutes per attempt | 30 seconds        | `activity_retry_policy()` | 12             | 1 second         | 2×      | 1 minute         |
| Manifest preparation | 1 hour per attempt    | 30 seconds        | `activity_retry_policy()` | 12             | 1 second         | 2×      | 1 minute         |

- These are current finite attempt budgets, not a guarantee of eventual success
  after every outage. Adapters distinguish permanent invalid input from transient
  failure. Policy factories do not implement private retry loops.
- Follow the [shared cancellation and recovery contract](durable-work-contract.md#cancellation-recovery-and-completion).
  Accept cancellation before apply/handoff or at a safe batch boundary, stop owned
  work, and do not launch handoffs after accepting cancellation. Downstream shared
  refreshes complete independently. Exhaustion remains visible for native/operator
  retry or existing reconciliation; no universal recovery ledger is required.
- Provider-directed `next_retry_delay` intentionally overrides computed backoff
  when Strava supplies a retry time. Clamp expired waits to the shared initial
  retry interval. Provider heartbeats use `ACTIVITY_HEARTBEAT_INTERVAL` (5 seconds),
  below the shared 30-second heartbeat timeout.
- Workflow executions and child executions intentionally have no automatic
  retry policy. Activities own effect retries; workflow history owns replay.
  Retrying a whole workflow creates a new run; domain idempotency must survive
  that new execution. Temporal's Workflow Task retry/replay behavior remains
  native. See [Temporal retry semantics](https://docs.temporal.io/encyclopedia/retry-policies).
- Verify emitted native Activity options against the shared policy in the
  existing Bike/Strava workflow helpers, including manifest heartbeat settings. Preserve fixtures and shared setup.
