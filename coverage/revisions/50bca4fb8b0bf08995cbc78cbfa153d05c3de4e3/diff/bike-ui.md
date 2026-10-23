# Diff Coverage
## Diff: db26266f8c9b4ebb0f461d80a0969e1b5a1643fa...HEAD, staged, unstaged and untracked changes

- bike-ui/components/segment-detail/SegmentDetailHeader&#46;tsx (100%)
- bike-ui/components/segment-detail/SegmentPersonalAchievements&#46;tsx (100%)
- bike-ui/components/segment-detail/useSegmentDetailState&#46;ts (0.0%): Missing lines 261
- bike-ui/components/SegmentDetailPanel&#46;tsx (100%)
- bike-ui/lib/queries&#46;ts (100%)

## Summary

- **Total**: 21 lines
- **Missing**: 1 line
- **Coverage**: 95.24%



## bike-ui/components/segment-detail/useSegmentDetailState&#46;ts

```
  257   reseedWhenSelectionEmpty: boolean;
  258 }) {
  259   const allEfforts = segment?.efforts ?? EMPTY_EFFORTS;
  260   const [selectionState, setSelectionState] = useKeyedState(
! 261     `${segment?.id ?? segmentId}:${initialSelectedEffortIds.join(",")}`,
  262     { ids: initialSelectedEffortIds, edited: false },
  263   );
  264   const availableEffortIds = new Set(allEfforts.map((effort) => effort.id));
  265   function resolvedSelection(state: { ids: number[]; edited: boolean }) {
```


---


