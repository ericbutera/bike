# UI Components Specification

Bike UI components should keep workflow behavior close to the component that owns the interaction. Parent components should compose workflows, not hold incidental modal state or helper functions that only exist to support one child dialog.

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

## Code Anchors

- Segment rename modal: `ui-next/components/SegmentRenameModal.tsx`
- Segment list UI: `ui-next/components/SegmentsPanel.tsx`
