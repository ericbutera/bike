# Activity list specification

- The signed-in rider's recent activities appear as cards with links to each
  activity's details, available summary metrics, timestamp, location, and
  achievement highlights. Format measurements using the rider's unit preference.
- Show a reusable, accessible sport icon immediately to the left of each
  activity title, with a tooltip naming the sport. Keep location and timestamp
  in one shared metadata layout with matching typography and text baselines
  when they appear on the same line. Allow metadata to wrap on smaller screens.
- Use Font Awesome Free solid icons for walking, hiking, road/generic cycling,
  running, indoor cycling, swimming, and workouts. Mountain biking and fat tire
  biking use original SVG artwork; no Pro assets or license are required.
- Recognize stored sport aliases for icon display. Unknown sports retain a
  neutral icon and their own readable label. The fat tire icon is available for
  explicitly labeled activities; do not infer tire type from a generic ride or
  change the supported sport/filter vocabulary for this presentation change.
- Provide an activity type filter with an **All activity types** option. Changing
  the filter returns to page 1. Keep the filter and current page in the URL so
  navigation restores the same results.
- The filter uses an icon-and-text menu, reusing the shared menu's keyboard
  navigation and dismissal. Show the current choice in the trigger and mark it
  checked in the menu; every sport option reuses the activity sport icon.
  Keep **All activity types** as the way to clear the filter.
- Request ten activities per page and use the
  [shared pagination contract](ui-components.md#pagination). Page changes
  preserve the filter and do not force a page scroll.
- Show distinct loading, error, and empty states. Offer the activity upload
  entry point from the feed as a compact square upload-icon link matching the
  shared Refresh button styling, with an **Upload Activity** label and tooltip.
- Route previews follow the [map image contract](maps.md#rider-experience).
  Sport visibility follows [supported activities](supported-activities.md).
- Selecting an activity opens its [activity details](activity-details.md).

## Saved card reads

- The feed queries only the authenticated rider's paginated activity scalar
  columns, saved card location, ordered preview points, and saved award rows.
  Do not select recording or heart-rate-zone JSON, expand a recording, reverse
  geocode, calculate ranks, or enqueue repair from the list GET.
- Activity metrics and editable titles/sports remain in their existing typed
  activity columns. `activity_cards` saves the route-derived location and
  displayed location. `activity_card_preview_points` saves at most 24 ordered
  latitude/longitude/elapsed-time samples per activity.
- The `rebuild_activity_card` processing operation reads the saved recording
  once. Reuse the existing preview selection: retain endpoints and coordinate
  extrema, then fill evenly sampled positions in original recording order.
  Produce cards for every supported sport, including successful empty previews.
- Save the card and its preview together after locking and checking the owned
  activity's source. A retry replaces the same keyed output; changed or deleted
  source data cannot publish an older result.
- Manual location edits update the displayed card location in the activity edit
  transaction. Clearing a manual location uses the already saved route location.
  Other metadata edits do not regenerate previews.
- Imports and reprocessing call the card operation as an independent derived
  branch. Preserve replay of histories recorded before this step was introduced.
- For the rollout, start `backfill_activity_cards` on `bike-tasks` in the `bike`
  namespace with JSON input `0`, using the existing Temporal UI or CLI. It pages
  all existing activities in ID order, at most 16 per page, and Continues-As-New
  only after each full page settles.
  It prepares cards without rematching, recalculating standings or changing
  saved awards. Repeat runs remain idempotent.
- Return currently saved values without timestamp/version gates. Activities with
  no saved card still appear with their scalar metrics, a null location and an
  empty preview until processing runs; a GET never invents missing output.
- The compact feed response contains card fields and saved earned awards.
  Detail-only recordings, laps, charts, training results and private metadata
  remain on the activity-detail endpoint. Preserve pagination, sport filtering,
  preview styling, and saved award-chip order.

LOAD14 owns this card cutover. LOAD15 separately owns prepared map images and
their compact ownership/cache lookup; card image requests are not part of this
read-model change.
