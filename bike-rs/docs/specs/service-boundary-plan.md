# Bike state and service boundaries

Task status is maintained only in [the Bike backlog](../../../docs/TODO.md). The original
progress checklist and test plan are preserved at `200921f` in Git. They are
historical migration records, not additional work for today's completion.

## Retained architecture

Bike Rust is one application with API, worker, migration, and shared
`bike-core` crates. Models/entities own queries, services own workflows, API
controllers adapt HTTP, and worker processors adapt durable jobs. The worker
calls core directly. Bike owns its platform modules without Kaleido.

Keep historical migrations append-only and run migrations as an explicit
release Job. Preserve stored data and queued payload compatibility. Version
breaking task changes through compatible defaults or a new task type while
old jobs drain. Keep transactions and idempotent effects at the owning boundary.

The Rust backend, UI, map renderer, and Strava gateway have independently
owned release boundaries. The gateway keeps its database and artifacts separate
from Bike application data. Contract and route checks run through the existing
explicit harness task.

## Release and verification references

The immutable API/worker/migration release pin and migration ordering are
recorded under DEP05/DEP06 in the backlog. The completed Rust boundary and
platform work is DONE02; the component test/build evidence is CI01/CI08.
The old requests to add payload, tag-drift, and architecture test harnesses
do not create another checklist.

For an actual boundary change, run the relevant existing Rust tests, formatting,
and lint tasks. Add a regression only for a demonstrated gap. Broad restart,
failed-migration, and rollback drills remain deferred
REC tasks; today's recovery work is REC08/REC09.
