# Staging fixtures

- The fixture CLI supports two explicit targets: disposable browser-test projects
  and `BIKE_FIXTURE_TARGET=staging` with `APP_ENV=staging`.
- Before staging writes, query the connected database name and require
  `bike_staging`. Never initialize the production database through this target.
- Reuse the browser baseline records, synthetic GPX files and heatmap preparation.
  The staging export substitutes the operator's SSO email and derives private
  API keys from the environment-supplied staging fixture key. Never publish the
  known browser-test API keys.
- Seed an empty database once. If its first user is the expected fixture owner,
  preserve existing data and finish any pending heatmap preparation. Reject a
  different existing owner. Fixture writes use the owning SeaORM models.
- After initialization, synchronize PostgreSQL sequences using the same SQL as
  browser tests. Normal application writes must not collide with fixture IDs.
- `mise run staging:fixtures` writes JSON to `BIKE_STAGING_FIXTURE_FILE`, using
  `BIKE_STAGING_OWNER_EMAIL` and `BIKE_STAGING_FIXTURE_KEY`. Keep this file private
  and temporary on the shared Docker disk.
- Pulumi owns the staging services, ingress, isolated persistent database/uploads,
  SSO client and Temporal namespace. Deploy the three accepted release digests
  from one clean tested revision without rebuilding them. Its owning deployment
  specification is `pulumi-iac/docs/specs/bike-staging.md`.
