# Segment Race Viewer Specification

The segment race viewer lets a rider watch selected segment efforts move against each other on the segment route. It is optimized for distinguishing nearby riders, understanding gaps as they open, and keeping each selected ride easy to track during playback.

## Product Intent

The race viewer should feel stable while the race is changing quickly. The map may follow the leaders, but rider identity, playback controls, and manually chosen viewing scale should not jump around in ways that make the rider lose context.

Downhill segments especially need close inspection because efforts can be short, fast, and tightly bunched. The viewer must make it practical to zoom in far enough to separate overlapping markers without the app immediately overriding that choice.

## Map Zoom

The viewer should expose map zoom as an intentional control, not only as an incidental map gesture.

When the rider sets the zoom manually, that zoom should stick during playback. Leader-follow behavior may continue to recenter the map, but it should not immediately replace the rider's chosen zoom level.

Manual zoom is especially important while markers are bunched. At high zoom levels, the viewer should preserve enough marker separation and label clarity for the selected efforts to remain identifiable.

On longer segments, the viewer may slowly scroll back out when a meaningful gap emerges between the top three efforts. This automatic zoom-out should be gradual enough to preserve visual continuity and should not fight a recent manual zoom choice.

Automatic zoom-out should be based on the top three live efforts, not only the first two. The target viewport should include the leading group when their positions spread out, while still avoiding sudden zoom jumps.

## Playback Speed

Playback speed should be controlled by a compact dropdown slider rather than the current coarse fast/slow button group.

The supported speed values are:

- `0.10x`
- `0.25x`
- `0.5x`
- `0.75x`
- `1x`
- `1.25x`
- `1.5x`
- `2x`
- `3x`
- `4x`

The selected speed should affect playback directly as a multiplier. `1x` means one race second advances per real second. The control should be usable on narrow and desktop layouts without forcing the timeline or timer into awkward wrapping.

## Rider Cards

Selected rider cards should keep a stable order during playback. They must not reorder based on live race position.

The initial order should match the selected effort order from the segment detail comparison. If the viewer seeds its own default selection, that default ordering should remain stable as well.

Cards may still display live position, lead status, and gap values, but those indicators should update in place. The rider should be able to keep their eyes on the same physical card location throughout playback.

## Live Gap Display

The viewer should continue to compute race position and leader gaps from live playback progress. Those calculations should drive marker following, leader/gap labels, and top-three zoom-out decisions.

Gap labels on cards should be derived from the current live leader even though the cards do not reorder. The current leader should be visually obvious without moving the leader card to the first position.

## Acceptance Criteria

- A rider can set a race-viewer zoom level and playback does not immediately reset it while following leaders.
- Manual zoom remains useful on downhill segments where selected markers are tightly bunched.
- Playback speed uses the fixed value set `0.10x`, `0.25x`, `0.5x`, `0.75x`, `1x`, `1.25x`, `1.5x`, `2x`, `3x`, and `4x`.
- Speed selection is presented as a compact dropdown slider control, replacing the fast/slow/auto button group.
- Selected rider cards stay in a stable order for the full playback session.
- Live position and gap labels update without moving cards.
- On longer segments, when the top three efforts spread apart, the map eases toward a wider viewport rather than snapping.
- Recent manual zoom input temporarily wins over automatic zoom-out.

## Code Anchors

- Race viewer page: `ui-next/app/segments/[id]/race/page.tsx`
- Race viewer UI: `ui-next/components/segment-detail/SegmentRaceViewer.tsx`
- Segment comparison helpers: `ui-next/lib/segmentDetail.ts`
- Route map UI: `ui-next/components/MapLibreRouteMapClient.tsx`
- Race viewer tests: `ui-next/components/__tests__/SegmentRaceViewer.test.tsx`
- Segment detail comparison tests: `ui-next/components/__tests__/SegmentDetailPanel.test.tsx`

## Implementation Notes

- Replace `PLAYBACK_PACE_OPTIONS` with a multiplier-based speed model or add a separate race-viewer speed model if the embedded segment-detail comparison should keep its current automatic pace behavior.
- Keep a sorted live-comparison collection for calculations, but render cards from the stable selected-row order.
- Extend leader-follow viewport calculation to account for the top three live markers when choosing automatic zoom-out.
- Track the timestamp of the most recent manual zoom interaction so automatic follow behavior can honor a cooldown before widening the viewport again.

## Open Decisions

- Decide the exact manual-zoom cooldown before automatic zoom-out is allowed to resume.
- Decide whether race speed should be persisted in the URL, local storage, or only component state.
- Decide whether zoom should be persisted per segment, per viewer session, or globally for the race viewer.
