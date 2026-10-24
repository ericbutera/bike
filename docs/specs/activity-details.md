# Activity details specification

## Scope

- Activity details show the stored activity summary, route, climbs, matched
  segments, training profile, signals, and laps when those data are available.
- This specification owns activity detail interactions. The
  [maps specification](maps.md) owns shared basemaps and default route styling;
  [segment processing](segment-processing.md) owns matching and effort data.
- Show a Font Awesome Free solid location-dot icon immediately before the
  location in the activity header. Keep the location text accessible and omit
  the location group when no location is available. Put the Free solid clock
  icon immediately before the header date/time. Keep both icons decorative,
  with their adjacent text accessible and vertically aligned.

## Saved data

Related implementation work follows the [loading checklist](../TODO-fix-data-loading.md#saved-reads-and-screens),
especially LOAD09-LOAD12 for saved ranks, facts, and prepared effort samples,
and LOAD22 for saved award-chip order.

The [DAG plan](performance.md#queries-each-screen-needs) distinguishes current
saved sources from the remaining producer cutovers:

| Detail data                     | Saved source / remaining producer                                                                                      |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Summary, metadata and laps      | Existing summary; P2 supplies typed metadata and ordered lap rows.                                                     |
| Route and signals               | P1 supplies the normalized recording and typed artifact reference.                                                     |
| Matched segments and attempts   | Existing `segment_efforts` and shared `segments`; P8 adds prepared per-effort facts/samples.                           |
| Earned awards and current ranks | Existing typed award rows, effort ranks and segment/rider summaries (#123/#126).                                       |
| Climbs                          | Existing `activity_climbs` plus saved report facts and typed artifact reference from #133; B3 still owns report reads. |

- The [saved climb read contract](performance.md#saved-climb-read-contract) owns
  the persisted climb metrics and prepared geometry. Read what is currently saved;
  activity JSON is not a read-time fallback.
- Use saved effort IDs, recording bounds and `effort_index` to display repeated
  attempts in their original order and retain their attempt numbers when ranked.
- Matching, climb detection, rank/award calculation and preparation of effort
  samples run in the DAG. Detail requests read those results instead of parsing
  the original upload, scanning riding history for ranks, redetecting climbs or
  extracting comparison samples from whole recordings.
- Read original award decisions even when current leaderboards have changed.
  Follow the [saved-order contract](performance.md#saved-ranks-and-display-order)
  for API ordering and UI rendering; neither layer reconstructs ranks or chip priority.
  Show the saved results while processing catches up. Timestamps, versions and
  checksums do not hide data or drive UI state. Opening an activity does not
  trigger request-time calculation as a fallback.

## Detail layout

- Place the title outside the cards, using the activity feed's **Recent
  activities** heading typography. Put location before time directly beneath
  the title. Replace the header's sport badge with the shared activity sport
  icon and its accessible label/tooltip. Make the sport icon and Training/Race
  tag clickable selectors that persist their own field and show save failures.
- Put **Edit activity** first in the header actions menu, followed by **Build
  segment** when route geometry is available, **Download original source**
  when available, and **Delete activity**. Do not show **Import details**,
  **Open Temporal workflows**, or **Regenerate derived data**, including for
  administrators and activities with regeneration capability.
- Give the four large summary stats their own card. Put **Activity data** in
  a separate compact card: a flat Avg/Max table for speed, heart rate, and
  cadence. Omit empty heart-rate/cadence rows. Put elapsed time, recorded gear,
  calories, start/end timestamps, and available private notes in the data grid.
  From 768px up, place the measurement table and details grid in two equal
  columns. On smaller screens, stack the details below the measurements.
  Keep description full-width below both sections and the details visible;
  omit Show more/Show less and sport/source/format/uploaded-file fields.
  Do not show help beside its heading.
- Use 16px between activity-detail cards. Keep the existing recorded values and
  user units; the reference layout does not introduce synthetic weather data.
- Edit activity supports title, sport, Training/Race, location, and description.
  When recorded gear or private notes exist, also offer the gear's display name
  and private notes. Preserve the gear identifier. These edits are stored in
  Bike. Preserve retained Strava `private_note` text and expose it to the owner;
  place private notes in Activity data's grid.
  Save only changed fields, preserve unknown stored sports unless changed, and
  retain the draft on save failure. Empty location restores route-derived
  location; empty description clears it. Format/source/upload provenance remain
  read-only.
- Show matched segments and detected climbs in one card with **Segments** and
  **Climbs** tabs and plain counts. Display one panel at a time, defaulting to
  Segments when matches exist and Climbs otherwise. Offer only tabs containing
  segment data. Keep the Climbs tab available even when no climb rows are saved;
  prefer a populated tab when choosing the default.
- Keep the existing segment rows and climb details inside their panels.
  Switching tabs preserves expanded segments, selected climbs, and segment
  selections. It does not change route-map layers, camera, or selection.
  A new activity starts with the default tab and fresh detail state.
- Support mouse and keyboard tabs with selected state, associated panels,
  roving focus, arrow keys, Home, and End. Hidden panels must not contribute
  page height or keyboard targets; render responsive charts only in the active
  panel. Keep the segment visual redesign outside this experiment.
- Combine each available signal's layer toggle, metric name, and summary value
  in one row above the chart. Keep summaries available when a layer is off;
  do not repeat them in badges below the chart. Preserve independent toggles,
  chart colors, and keyboard access. Hide the signals card when there are no
  usable series; when available layers are all off, retain the controls and
  show the prompt to turn one on.
- Render lap splits as one compact table: lap number, name, time, distance,
  average speed, average heart rate, and maximum heart rate. Use the rider's
  units and the usual missing-value formatting. Show the lap count as plain
  text; use no duration badges or nested metric cards for laps. Keep the
  explanatory empty state for uploads without explicit laps.
- Identify selected climbs by the same one-based number as their list row
  (**Climb 1**, **Climb 2**, etc.). Label the start/end distance separately as
  **Activity distance** so a climb starting at zero is not read as Climb 0.
- Keep selected-climb metric values smaller than the detail heading. Fit the
  elevation profile's horizontal domain to its first and last distance samples,
  without extending it to a rounded tick. Keep the entire profile visible on
  narrow screens, and preserve units, tooltips, and climb selection behavior.

## Route map and matched segments

- Activity detail reads current ranks directly from the rider's saved segment
  efforts and personal-best durations from their saved segment/rider summaries.
  Limit queries to the requested activities and their matched segments.
- Return saved values unchanged while background processing catches up. Missing
  ranks or personal-best summaries remain null; timestamps never hide saved
  results. GET requests do not scan effort history, calculate replacements,
  write results, or enqueue repair.
- Preserve effort traversal order and the separately saved earned-award decisions
  and chip display order. Current ranks do not recreate historical awards.
- Matched segment lines use the same 6 CSS pixel width and 9 CSS pixel white
  casing as the interactive blue activity route. Hovering or selecting must not
  thicken the lines. Render focused segments above other segments.
- The overall route map always displays the blue activity route. Matched
  segments are on by default; detected climbs are off by default. Selecting a
  climb row shows its metrics and elevation profile within the Climbs panel
  without controlling the overview camera.
- Route-map segment controls use soft gray borders in both selected and
  unselected states, retaining their colored status dots and selection behavior.
- Assign colors in alphabetical segment order, breaking ties by segment ID.
  Repeat this ten-color palette for activities with more than ten segments;
  every effort of a segment shares its color:

  | Color    | Hex       |
  | -------- | --------- |
  | Coral    | `#f43f5e` |
  | Violet   | `#8b5cf6` |
  | Teal     | `#00a6a6` |
  | Pink     | `#ec4899` |
  | Cyan     | `#06b6d4` |
  | Orange   | `#f97316` |
  | Lime     | `#84b51b` |
  | Lavender | `#a78bfa` |
  | Mint     | `#10b981` |
  | Magenta  | `#d946ef` |

- Place a flat, 12px daisyUI status circle immediately before each segment
  title in both the map legend and the matched segment detail headers. Its
  exact color matches the segment line. Keep these circles in their assigned
  colors during map focus changes and keep the legend pills neutral.
- Initially show all segment colors. Hovering a segment line or its map legend
  button temporarily shows only that segment in color; all other matched
  segments use the default Bike blue. Keyboard focus on a legend button offers
  the same preview. Leaving the preview restores all current selections,
  including after clicking another map line while a preview is active.
- Clicking a segment line or legend button toggles that segment's persistent
  selection without scrolling the page or moving focus to its detail card.
  Allow multiple selections. Outside a hover/focus preview, selected segments
  retain their assigned colors and all other segments become blue.
- When segments are selected, place a regular daisyUI button inline at the end
  of the segment bubbles, with a leading Font Awesome **rectangle-xmark** icon
  and visible **Clear selection** text. Its accessible label and tooltip also
  identify the selected count. Hide it when nothing is selected. Keep the
  legend free of hover instruction text.
  Clearing removes the persistent selection and any temporary preview,
  restoring all assigned colors. Deselecting the final segment also restores
  all assigned colors once the preview ends. New activities start with no
  selection.
- Moving across a row of segment bubbles continuously previews the current
  bubble, including during fast uninterrupted sweeps. Map hit-testing and older
  enter/leave events must not replace the current legend preview. Apply the
  latest preview even while earlier map geometry updates are still processing.
- Selection stays synchronized with the matched segment details. Preserve their
  expansion and keep map camera position
  stable during segment selection or hovering.
- Starred segments start expanded but can be collapsed and reopened with the
  same pointer/keyboard controls. Collapsing does not remove the star or alter
  map selection.

## Route map layers

- Place a layers icon directly below the zoom in/out buttons in the same map
  control stack. Its **Map layers** popup contains independent **Segments** and
  **Climbs** checkboxes. Both, either, or neither overlay can be shown.
- Hiding segments hides their map legend and colored overlays while retaining
  the selected segment set. Showing them again restores that selection. Layer
  changes preserve the blue route, camera position, and climb detail selection.
- Layer visibility belongs to the current activity view. New activities start
  with segments on and climbs off. Theme changes preserve the current choices.
- Draw climbs as narrow white-cased lines alongside the route so overlapping
  segments remain distinguishable. Use an orange `#fb923c` to coral `#f43f5e`
  to purple `#a855f7` gradient from each climb's start to finish. This represents
  progress along the climb, not local grade or an intensity measurement.
- Repeat arrows along each climb in its recorded riding direction, including
  on reversed and curved geometry. Keep arrow colors readable in both themes.
  Show a **Climb start → finish** gradient key while climbs are visible.
- Support keyboard activation of the layers button and checkboxes. Escape
  closes the popup and returns focus to the layers button; clicking outside
  closes it without moving focus away from the clicked target.

## Climbs and focused maps

- Show the climb count as plain text beside the card heading, without a pill.
  Do not offer a **Zoom out map** action in the climb details.
- Show twenty climb rows at a time, with vertical scrolling for additional climbs.
- Each climb row and matched segment entry offers a Font Awesome **route** icon
  button, with a **View [name] on map** accessible label and tooltip. The button
  has no visible **View on map** text. This opens a separate, titled map dialog
  fitted to the relevant stored route section.
  A climb uses its detected route points; a matched segment uses its best
  matched effort's route section and retains its assigned segment color.
- Focused segment and climb maps use only the shared green play start icon and
  checkered finish flag (`faFlagCheckered`), with no buttons, text labels, or
  background badges. Both icons are 16px and centered directly over the first
  and last points of the displayed section, including when they share a location.
- Focused segment and climb maps show the same white direction arrows as the
  main route map, following the stored route order through curves and reversed
  sections. Keep the arrows visible above the colored route, with a halo that
  matches that route's color.
- Keep the overall route map's camera, matched segment selection, and climb
  detail selection unchanged when opening or closing a focused map. These
  actions do not scroll the page to the overall map or expand a segment card.
- Reuse the standard modal's focus management, Close button, backdrop dismissal,
  and Escape behavior. Mount the focused map only while its dialog is open.
  Disable the route icon button when fewer than two stored route points are
  available.

## Saved climb data

- The climb worker saves duration, distance, elevation gain/loss, average and
  maximum grade, category, recording bounds, and prepared route/profile points
  in `activity_climbs`. Reuse the existing climb operation and backfill.
- Activity detail and update responses expose the currently saved climbs.
  Summary/list responses do not load climb output.
- API reads select saved rows. They do not decode, hash or slice the activity
  recording, rerun detection, or reject saved rows because processing metadata
  is old or missing.
- The UI displays saved metrics and renders prepared geometry/profile points.
  It only formats units and labels; it does not calculate climb metrics.
- Missing or empty saved rows show **No sustained climbs found.** Keep the Climbs
  tab available with its saved count, including zero. Do not create a processing
  availability state or hide stored results using timestamps, versions or checksums.
- Existing rows retain their saved metrics while processing catches up. New
  nullable metrics use the usual missing-value display, and missing prepared
  geometry omits the map/profile without removing the climb.
- Processing output version 2 populates the new saved fields through the existing
  `backfill_activity_climbs` operation. Versions never gate API/UI reads.
- The same saved climbs supply the list, details, profiles, focused maps and
  overall route overlays. Preserve units, selection and layer controls.

## Climb identification

- Numbered climbs identify detected route sections within this activity. They
  are not implicitly named or matched segments.
- More useful climb labels and any association with matched segments remain an
  open product decision, tracked in [the activities backlog](../TODO.md#activities).
  Preserve the current climb data and separate map access while that is decided.

## Acceptance

- Cover activities without signal samples in the shared empty-state browser
  check by asserting that the **Ride signals** region is absent.
- Verify the ten distinct colors over the muted green light basemap and the
  dark basemap, with matching legend status dots and white line casings.
- Exercise activities with more than twenty matches, repeated efforts, and
  overlapping segments; the hovered or selected segment must remain visible.
- Require unit tests with shared fixtures and mocked map rendering for hover
  previews from map lines and legend buttons, restoration after leaving a
  preview, and keyboard focus previews.
- Require unit regressions for fast forward and reverse sweeps across more
  than twenty segment bubbles, stale enter/leave events, and the latest
  highlight arriving while earlier map source updates are still processing.
- Require unit tests for selecting multiple segments, toggling an individual
  selection off, restoring selections after hover, and clearing while a preview
  is active. Exercise map-line and legend selection where applicable.
- Verify that the clear button is the last item in the bubble list, appears only
  with selected segments, shows its icon and **Clear selection** text, has its
  accessible label and tooltip, and disappears after clearing. Verify flat 12px
  status circles and neutral pills in both themes.
- Verify keyboard selection, reset when navigating to another activity, and no
  page jump or camera refit when selecting matched segments.
- Use an activity containing a sustained climb to verify the default layer
  visibility, all four layer combinations, selection restoration, gradient
  direction, arrows, theme switching, and a stable overview camera.
- Allow gradient interpolation and raster antialiasing in rendered climb colors.
  Unit fixtures verify the directional arrow image and placement; browser checks
  require the climb gradient to appear and disappear with its layer.
- Exclude map-control focus styling from overview image comparisons so keyboard
  focus changes do not masquerade as camera or route changes.
- Selecting a climb row only changes climb details; its route icon shows the
  correct section in an independent dialog.
- Verify focused map opening, color, dismissal, missing geometry, and no changes
  to overview selection, expansion, or camera for climbs and matched segments.
- Require unit coverage for focused route colors and direction arrows, including
  their layer order and route-colored halo. Verify that changing a route's color
  preserves its geometry and camera framing.
- Verify the green start icon and checkered finish flag in both focused dialogs
  and on the overall activity map, including their smaller size and common
  center on closed routes.
