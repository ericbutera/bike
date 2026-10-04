# Shared contract examples

These synthetic JSON examples contain no production data. Rust's code and the
[generated OpenAPI contract](../openapi) define current behavior. The examples
describe request and response shapes; they are not a separate acceptance suite.
Native unit, functional, and integration fixtures remain with their owning
tests.

| Example                                                                                                                                                | Boundary                  |
| ------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------- |
| [Activity list query](activity-list-query.json), [response](activity-list-response.json)                                                               | Activity pagination       |
| [Activity update](activity-update-request.json)                                                                                                        | Activity mutation request |
| [Invalid activity path](invalid-activity-path.json)                                                                                                    | Request validation        |
| [Unauthenticated](unauthenticated-error.json), [not found](not-found-error.json), [conflict](conflict-error.json), [dependency](dependency-error.json) | Typed error responses     |
| [Process activity import](process-activity-import-task.json)                                                                                           | Durable task payload      |

Intentional build-context mirrors and their synchronization commands are
recorded in [the shared asset inventory](../../docs/shared-assets.json).
