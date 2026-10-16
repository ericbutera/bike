# Bike API test fixtures

- API unit tests use shared fixtures and PostgreSQL SeaORM mocks in
  `api/src`. Run focused cases with `mise --cd bike-rs exec -- cargo test -p api --lib <filter>`.
- The SQLite-backed `platform.rs` suite and its schema helpers have been removed.
  Do not use SQLite to model PostgreSQL timestamps, locks, transactions, or JSON queries.
- Existing PostgreSQL checks and the Docker browser suite own database and
  service integration evidence. Mock tests establish unit behavior only.
- `fixtures/platform/uploads/` retains synthetic activity and segment uploads
  used by the Docker browser suite.
- Browser scenario data lives in
  [Playwright seed builders](../../../bike-ui/tests/e2e/helpers/seeds.mjs), is
  validated through current ORM models by `e2e-fixtures`, and is snapshotted after
  current migrations. Browser tests restore data while services remain running.
