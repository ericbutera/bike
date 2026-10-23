# Diff Coverage
## Diff: efd54987678c9b204304feccf9a7651776ca2334...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_climbs&#46;rs (99.5%): Missing lines 138
- bike-rs/bike-core/src/entities/activity_climbs&#46;rs (100%)

## Summary

- **Total**: 250 lines
- **Missing**: 1 line
- **Coverage**: 99.6%



## bike-rs/bike-core/src/activity_climbs&#46;rs

```
  134     let (start_distance, start_elevation) = distance_and_elevation(&pair[0])?;
  135     let (end_distance, end_elevation) = distance_and_elevation(&pair[1])?;
  136     let distance = end_distance - start_distance;
  137     if distance <= 0.0 || pair[1].elapsed_seconds <= pair[0].elapsed_seconds {
! 138         return None;
  139     }
  140     Some((end_elevation - start_elevation) / distance * 100.0)
  141 }
```


---


