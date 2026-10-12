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

## List presentation

- `ListCard` owns the common list title and right-aligned filter toolbar, with
  compact inputs, dropdowns, Search, and Clear above its content.
- `DataGrid` owns zebra rows, column rendering, loading/error/empty states,
  horizontal overflow, and optional row selection. Long cell values use
  truncation with a tooltip or separate columns.
- `GenericList` combines these components with URL filters and pagination;
  pages with independent filters reuse the same presentation components.

## Code Anchors

- Shared numbered pagination: `bike-ui/components/ui/Pagination.tsx`
- Segment rename modal: `bike-ui/components/SegmentRenameModal.tsx`
- Segment list UI: `bike-ui/components/SegmentsPanel.tsx`
- Shared grid: `bike-ui/components/ui/DataGrid.tsx`
- URL-driven list: `bike-ui/components/ui/GenericList.tsx`
