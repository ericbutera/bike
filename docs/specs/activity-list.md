# Activity list specification

- The signed-in rider's recent activities appear as cards with links to each
  activity's details, available summary metrics, timestamp, location, and
  achievement highlights. Format measurements using the rider's unit preference.
- Provide an activity type filter with an **All activity types** option. Changing
  the filter returns to page 1. Keep the filter and current page in the URL so
  navigation restores the same results.
- Request ten activities per page and use the
  [shared pagination contract](ui-components.md#pagination). Page changes
  preserve the filter and do not force a page scroll.
- Show distinct loading, error, and empty states. Offer the activity upload
  entry point from the feed.
- Route previews follow the [map image contract](maps.md#rider-experience).
  Sport visibility follows [supported activities](supported-activities.md).
- Selecting an activity opens its [activity details](activity-details.md).
