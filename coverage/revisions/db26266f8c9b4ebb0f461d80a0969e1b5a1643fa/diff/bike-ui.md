# Diff Coverage
## Diff: efd54987678c9b204304feccf9a7651776ca2334...HEAD, staged, unstaged and untracked changes

- bike-ui/components/activity-detail/ActivityClimbsPanel&#46;tsx (75.0%): Missing lines 93
- bike-ui/components/activity-detail/ActivityRouteDetails&#46;tsx (100%)
- bike-ui/components/activity-detail/ActivityRouteMap&#46;tsx (100%)
- bike-ui/lib/activityClimbs&#46;ts (100%)

## Summary

- **Total**: 18 lines
- **Missing**: 1 line
- **Coverage**: 94.44%



## bike-ui/components/activity-detail/ActivityClimbsPanel&#46;tsx

```
  89     return [
  90       {
  91         distance:
  92           unitSystem === "imperial"
! 93             ? (point.distance_meters - (climb.startDistanceMeters ?? 0)) /
  94               METERS_PER_MILE
  95             : (point.distance_meters - startDistanceMeters) / 1000,
  96         elevation: elevationChartValue(point.elevation_meters, unitSystem),
  97       },
```


---


