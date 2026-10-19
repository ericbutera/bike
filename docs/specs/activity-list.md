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
