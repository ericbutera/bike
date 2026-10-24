# Segment matching fixtures

The City and Commons recordings together cover every segment matched on activity 1671. Activity recordings already checked into this directory are reused.

`city_efforts.json` freezes the established matcher on `main` at
`ee96f2889d30e18f4fe3cd9355239da407b6da9b`, before P5 candidate-query changes.
The existing City recordings correspond to activities 1655 (`city_01.fit`,
2026-08-08) and 1660 (`city_02.fit`, 2026-08-15). Saved efforts were captured
read-only on 2026-10-24 to cross-check the fixture baseline. Segment GPXs missing
from the original corpus use fixed segment geometry captured on the same date.

Each window is `[start point index, end point index, start elapsed seconds, end
elapsed seconds, duration seconds]`; an empty array means no match. Ordered
windows retain repeated attempts. Keep these expectations independent of future
implementations; do not regenerate them to make a changed matcher pass.

The current P3 matcher preserves two traversals absent from the older saved City
results: the second Hickory Hills To Hickory Meadows lap (18586–19129 seconds)
and the first Ashes of the Wake lap (7032–8042 seconds). Both follow the fixed
segment geometry (maximum segment-to-recording nearest-point offsets below
20 meters). The second Ashes lap ends at 17998 seconds when matching the GPX's
calculated distances, versus 17997 in the saved provider recording. These fixture
expectations preserve the established P3 behavior: 18 City 1 efforts and 21 City 2
efforts. Exact activity 1671 persistence acceptance now uses the [shared DAG fixture](../activity_dag/README.md).
The ordinary unit suite parses the real FIT and GPX files and compares membership
and ordered windows. See the [matching specification](../../../../docs/specs/segment-processing.md#shared-real-ride-regressions).

unmarked_01.fit:

- F-BOMB OUT
- Jam Sesh
- Adderall in Reverse
- Not-So Holy Grail (EB)
- The Holy Grail Single Track

unmarked_02.fit:

- VST Clockwise 2020 (6 to 5 - Saplings)
- F-BOMB IN
- Rally Back CBS Rework

commons_01.fit:

- Breakin The Law
- FMR
- FMR Full
- Log Jam

city_01.fit (18 fixture efforts):

- Breakin The Law
- -g
- The Hick's Descent
- Hickory Hills To Hickory Meadows
- Bacon Tomato Lettuce (BTL short)
- root hill
- Hickory forest, up we go...
- Hickory Meadow's Wayne Hill Backside
- Classic North DH
- Old Orchard Luge Descent
- Short and steep
- Misdemeanor

city_02.fit (21 fixture efforts):

- East Side Flow
- Country Club Boys
- -g
- The Hick's Descent
- Breakin The Law
- Bacon Tomato Lettuce (BTL short)
- Classic North DH
- Old Orchard Luge Descent
- Misdemeanor
- Coward Log Connector
- Ashes of the Wake

short_connector_nearby_route.gpx:

- Short-connector false-positive regression; this nearby route window must not match `short_connector_segment.gpx`.
