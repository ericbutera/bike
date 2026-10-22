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

## Segment competition

- Keep the small Northbound baseline. Add a separate `segment-competition`
  scenario to staging. The ordinary browser baseline stays small.
- Use 15 active competitors: the SSO reviewer and 14 fictional riders. Each
  competitor rides the same five segment records, with real geometry from
  `api/tests/fixtures/segment_matching` and fictional dates and timing.
- Use `city_02.fit` with Breakin The Law, Country Club Boys and East Side Flow;
  use `unmarked_01.fit` with Jam Sesh and The Holy Grail Single Track.
- Retain generated GPX inputs so imports can be reprocessed. Parse and match
  through the existing Bike functions, then calculate current summaries and
  prepare heatmaps. Do not seed hand-written ranks or achievements.
- Include the reviewer earning tenth then losing current tenth later that day,
  successive KOM holders, more than ten personal attempts, repeated attempts
  in one ride, and distinct day/week/month/year results across 2024–2026.
- On an existing staging database, append the competition scenario once without
  resetting the baseline or other data. Commit its database rows together;
  reject conflicting fixture IDs. Keep the existing staging database/owner guards.
- Current calculations remain unchanged. The fixture data establishes known
  cases for the planned unique-athlete leaderboards and permanent historical
  achievements; seeding successfully does not claim those features exist yet.

### Review cases

The competition adds 46 rides. User `1` is the existing reviewer; users
`101`–`114` are fictional active competitors. Segment IDs `4101`–`4103` are the
City Loop segments above, in that order; `4104` and `4105` are the Unmarked
segments. Names start with `Fixture` so they are easy to find.

| Ride                                  | Scenario                                                       | Expected result after the pipeline changes                                                                                             |
| ------------------------------------- | -------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `31001`, June 1, 2025, morning        | Reviewer earns all-time tenth on the three City Loop segments. | Each effort retains its earned top-10 achievement and PR.                                                                              |
| `31002`, same day, afternoon          | Rider 110 beats the reviewer's time.                           | Reviewer moves to current eleventh; ride 31001 keeps its awards.                                                                       |
| `31003`, June 2, 2025                 | Rider 102 beats rider 101's KOM.                               | Current KOM belongs to 102; the earlier KOM remains on 101's ride.                                                                     |
| `31004`, June 1, 2026                 | Reviewer rides slower than their 2025 best.                    | Reviewer leads 2026, but earns no all-time PR or KOM for this ride.                                                                    |
| `31005`, July 6, 2026                 | Reviewer rides City Loop three times.                          | Every matched attempt is separately selectable in detail comparison and Race Viewer; overall leaderboard still has one reviewer entry. |
| `31100`–`31109`, January–October 2024 | Ten earlier City Loop rides by the reviewer.                   | Personal top 10 spans multiple rides; Segment Progress shows separate 2024, 2025 and 2026 bests.                                       |

The expected final all-time athlete order for the City Loop top 10 is
`102, 101, 103, 104, 105, 106, 107, 108, 109, 110`; the reviewer is eleventh.
The source City Loop crosses Breakin The Law twice; the three repeats in ride
`31005` therefore produce six efforts on that segment, each selectable separately.
Select the fixed dates above when checking day/week/month/year filters. June
and July 2026 intentionally have different personal bests.

The existing PostgreSQL test task runs the Rust fixture test. It loads the same
staging export, runs the normal parser, matcher and result functions, and calls
the segment detail, comparison, yearly-best and effort-analysis endpoints.
Assertions check those application responses, including rider/activity attribution
and repeated attempts. Heatmap checks use the owning application queries.
Repeat loading must preserve effort IDs and existing edits.
CI collects this database test's coverage alongside unit coverage before checking
the existing changed-line coverage requirement.

Historical awards and unique-athlete leaderboard assertions belong with their
pipeline implementation and use the same fixture cases above.
