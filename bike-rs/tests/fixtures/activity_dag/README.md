# Activity 1671 regression

`activity-1671.json.gz` contains the exact 13,114 point recording and 33 segment
geometries captured read-only on 2026-10-24 at production revision
`47148573658c85b6eaca34b7da333d486462a2b1`. Compression keeps the fixture small;
`gzip -dc activity-1671.json.gz` reads the ordinary JSON dataset.

The 17 saved efforts across 16 segments, both Coward Log Connector attempts,
12 climbs and training/speed facts are independent reference outputs. Do not
regenerate expectations from a changed implementation to make a test pass.

Provider/customer identities, description, gear and private notes are
synthetic. Raw private provider extensions, credentials, historical rider records
The harmless original title `Full stop` is retained because route-family classification uses it.
and import paths are excluded. Recording samples and segment geometry are
unchanged. Segment IDs use a fixed offset for isolation from the normal browser
fixtures. Only heart-rate zones needed by the training calculation are retained.

The connected regression uses the existing E2E PostgreSQL database on tmpfs and
Temporal server. It runs registered Strava intake and Bike workflows with a fake
provider, starts with no recording, metadata, climb, card or standings output,
and verifies an unchanged second event preserves effort identities and awards.
The old award decision is seeded as legacy conversion input; its five highlights
must survive. No additional CI job or service stack is introduced.
