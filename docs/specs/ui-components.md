# UI Components Specification

Bike UI components should keep workflow behavior close to the component that owns the interaction. Parent components should compose workflows, not hold incidental modal state or helper functions that only exist to support one child dialog.

## Pagination

- Numbered pagination uses the shared `Pagination` component for activity,
  generic list, admin task, import history, and segment effort lists.
- Show up to five page numbers by default, centered on the current page where
  possible and shifted to stay within the first and last pages.
- Double-left `«` jumps directly to page 1; double-right `»` jumps directly to
  the last page.
- Single-left `‹` moves back one page; single-right `›` moves forward one page.
- Place first/previous controls before the numbers and next/last controls after
  them. Give each arrow an accessible label describing its destination.
- Hide first/previous controls on page 1 and next/last controls on the last page.
- Hide all arrow controls when every page number fits in the visible range,
  including when a caller customizes the number of visible pages.
- Hide pagination when the result fits on one page. Mark the selected number
  with `aria-current="page"`; selecting it again does not request a page change.
- A first-page jump must restore page 1 data and the owning page's URL state,
  including when page 1 is outside the visible number range on page 5.
- Cursor-backed lists use known page numbers plus the next available page,
  retaining visited cursors for backward navigation. Hide the last-page jump
  because the final page is unknown. Disable paging during requests.

## Modal Behavior

Modals should be self-contained by default.

A modal component should usually own:

- its trigger button or trigger control;
- open and close state;
- local draft form state;
- validation needed before save;
- mutation calls for the workflow it represents;
- success/error toast behavior specific to that workflow;
- disabled/loading state while saving;
- backdrop and cancel handling.

Parent components should usually pass the domain object or id the modal operates on, for example `<SegmentRenameModal segment={segment} />`. The parent should not need separate `isOpen`, `draftTitle`, `openRenameModal`, `closeRenameModal`, or `saveSegmentTitle` state/functions for a modal whose behavior is not shared elsewhere.

Passing hooks or callbacks from the parent is acceptable only when the parent genuinely owns extra behavior, such as refreshing unrelated state, coordinating multiple modals, tracking analytics, or overriding save behavior for a different workflow. Do not add those extension points speculatively.

## Composition Contract

List, card, and detail components should stay focused on layout and domain composition. If a repeated row needs a rename, delete, import, or edit dialog, prefer a small workflow component that renders its own button and modal together.

The self-contained workflow component should have a narrow prop surface:

- the object or id being acted on;
- optional display context when needed;
- optional lifecycle callbacks only after a concrete caller needs them.

Tests should exercise the workflow through the visible trigger and dialog controls. A parent component test may cover the integrated behavior, but implementation details such as parent-owned modal draft state should not be required.

## Data grids

- These requirements apply to every data grid, including admin and user pages.
- Filters must match the established `/admin/users` presentation: compact inputs
  and dropdowns in a right-aligned toolbar above the grid, with consistent
  sizing, spacing, Search, and Clear controls. `ListCard` owns this shared
  toolbar. Changing or clearing filters resets pagination to the first page.
- Admin Refresh controls use the Font Awesome arrows-rotate icon, retain an
  accessible Refresh label, and sit after Search and Clear at the far right of
  the filter toolbar. Detail pages without filters place it at the right of
  their heading.
- `DataGrid` owns zebra rows, column rendering, loading/error/empty states,
  horizontal overflow, and optional row selection. Long cell values use
  truncation with a tooltip or separate columns.
- Admin grids place date and time columns after ordinary data columns, followed
  by all button and action columns at the far right. Activity Source values
  remain plain text.
- The Users grid includes Created and never reads audit events.
  Its row menu separates Viewing, Editing, and Task management. View details
  loads account details and the last recorded successful sign-in on demand;
  View audit events opens that user's paginated audit history on demand.
  Session refreshes do not change Last login; missing history is shown as an em dash.
- Use plain text for ordinary cell values. Reserve pills and badges for
  meaningful statuses or categories; do not wrap every value in a pill. Event
  labels, users, IDs, and connections should normally remain plain text.
- Admin status, outcome, stage, and event-level cells share the filled
  `StatusBadge` presentation from Users: success is green with dark text,
  failures are red, pending or canceled states are yellow, and work in progress
  is blue. Unrecognized states use a neutral pill.
- Brands must use their favicon or official brand icon instead of a text pill.
  Keep icons small enough for condensed rows, normally 16 pixels, and provide
  an accessible brand name.
- Grids must use the shared `Pagination` control below the table and follow the
  [pagination rules](#pagination), including cursor support. Hide the control
  only when all results fit on one page.
- Data grids must never poll. Do not configure refresh intervals, timers, or
  repeated background requests for grid data. Disable automatic refetch on
  window focus and reconnect. Load initially, on Search or Refresh, when paging
  or filters change, or after a completed mutation affects the grid.
- `GenericList` combines these components with URL filters and pagination;
  pages with independent filters reuse the same presentation components.

## Admin task menus

- Retire `/admin/manual-tasks` and its navigation. The Users gear menu owns
  Fetch provider export archives, Regenerate segment efforts, and Clean up
  duplicate activities. Resolve the affected user from the row; never ask for a
  user ID. Segment effort regeneration reuses the user-wide segment workflow.
- Archive imports ask for a provider export URL and call administrator-only
  `POST /admin/users/{id}/archive-imports`; use the target's identity and storage
  key, return a queued import, and link to its import history. Duplicate cleanup
  reuses the existing user-scoped endpoint and shows its completed summary.
- Admin grid gear menus share one accessible action-menu component. Render the
  popup outside the grid's horizontal overflow so the last row is not clipped.
  Support keyboard focus, arrow navigation, Escape and outside dismissal. Gear
  and menu clicks must not open the Users row's account-edit dialog.
  Keep the popup anchored when the grid or page scrolls, and focus menu items
  without scrolling the page; opening an offscreen action column must not
  immediately dismiss the popup.
- Each action owns its request dialog and pending/error/accepted state. Show the
  selected target and task-specific options; a backfill asks for dates, retained
  import reprocessing offers archive FIT only. Preserve a failed request's
  command UUID when retrying unchanged parameters and show the native job link
  after acknowledgment.

## Admin integration events

- Replace inline Details with a small, labeled inspect icon. Its shared modal
  preserves the message, metrics, Temporal link, and expandable JSON payload.
  Closing returns to the list; account event history keeps its existing layout.

## Code Anchors

- Shared numbered pagination: `bike-ui/components/ui/Pagination.tsx`
- Segment rename modal: `bike-ui/components/SegmentRenameModal.tsx`
- Segment list UI: `bike-ui/components/SegmentsPanel.tsx`
- Shared grid: `bike-ui/components/ui/DataGrid.tsx`
- URL-driven list: `bike-ui/components/ui/GenericList.tsx`
- Grid actions and task dialogs: `bike-ui/components/admin/AdminTaskMenu.tsx`
