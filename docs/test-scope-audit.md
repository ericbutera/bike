# Bike verification scope

Use the owning component's existing suite for the behavior being changed.
The normal platform browser check covers activity list/detail, segment
list/detail, and race playback as one connected journey. Broader browser runs
are available when an observed bug needs them.

## Test boundaries

| Boundary                           | Preferred evidence                                                       |
| ---------------------------------- | ------------------------------------------------------------------------ |
| Parser and analytics rules         | Rust unit tests and one relevant activity fixture                        |
| Persistence and workflow           | Owning service/model tests; PostgreSQL when its behavior matters         |
| UI state and controls              | Existing React functional tests with query/router fakes                  |
| Provider adapter                   | Gateway tests with fake HTTP transport and supported response fixtures   |
| Provider callback persistence      | Gateway integration tests with a disposable PostgreSQL schema            |
| Connected application              | Focused Playwright activity, segment, and race journey                   |
| API health and known fixture reads | Native Rust HTTP integration test with required activity/route/race data |
| Recovery                           | An actual saved database/file restore with retained IDs and hashes       |

## Recorded functional migrations

TEST06 moved failed-import display assertions into
[ActivityImportsPanel.test.tsx](../bike-ui/components/__tests__/ActivityImportsPanel.test.tsx).
The case verifies the failed row, status, diagnostic, and available actions with
an existing query fake. Its recorded run passed all 16 component cases.

TEST07 moved fabricated pagination into
[ActivityStream.test.tsx](../bike-ui/components/__tests__/ActivityStream.test.tsx).
The case verifies page-one content, page-two selection and URL, Previous, and
removal of the page query. Its recorded run passed all eight component cases.
These are historical source-check results, not a fresh run against today's tree.

The connected platform synthetic retains real product APIs, SQL, and the owned
map renderer. It fakes external basemaps. Native Rust HTTP integration tests
check seven reads, invalid bearer rejection, and preference updates through the
real Axum router and SeaORM against an isolated SQLite fixture. Reads require
the seeded activity, route points, and both race efforts, so empty responses
cannot pass.
See [API tests](../bike-rs/api/tests/README.md) and
[UI browser tests](../bike-ui/tests/e2e/README.md).

## Evidence limits

Source registrations and generated-client freshness establish contract wiring.
They do not establish live authentication, provider sync, database resource use,
or production health. Record whether data, provider calls, or application APIs
were faked. Compilation alone does not complete the pending workflow, SQL, and
memory work in the [backlog](TODO.md).
