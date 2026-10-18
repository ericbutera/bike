# Activity details specification

## Scope

- Activity details show the stored activity summary, route, climbs, matched
  segments, training profile, signals, and laps when those data are available.
- This specification owns activity detail interactions. The
  [maps specification](maps.md) owns shared basemaps and default route styling;
  [segment processing](segment-processing.md) owns matching and effort data.

## Route map and matched segments

- Matched segment lines use the same 6 CSS pixel width and 9 CSS pixel white
  casing as the interactive blue activity route. Hovering or selecting must not
  thicken the lines. Render focused segments above other segments.
- The overall route map always displays the blue activity route. Matched
  segments are on by default; detected climbs are off by default. Selecting a
  climb row shows its metrics and elevation profile within the climbs card
  without controlling the overview camera.
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
  existing expansion and starred-segment behavior and keep map camera position
  stable during segment selection or hovering.

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
- Show five climb rows at a time, with vertical scrolling for additional climbs.
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

## Climb identification

- Numbered climbs identify detected route sections within this activity. They
  are not implicitly named or matched segments.
- More useful climb labels and any association with matched segments remain an
  open product decision, tracked in [the activities backlog](../TODO.md#activities).
  Preserve the current climb data and separate map access while that is decided.

## Acceptance

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
