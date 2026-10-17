# Diff Coverage
## Diff: a0f7c70dcc2234f137737cc4f957161bf1e7e5fd...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-temporal/src/client&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/debounce&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/prepare_heatmap&#46;rs (87.5%): Missing lines 27
- bike-rs/bike-temporal/src/workflows/rebuild_fitness_freshness&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/rebuild_segment_analytics&#46;rs (87.5%): Missing lines 27

## Summary

- **Total**: 44 lines
- **Missing**: 2 lines
- **Coverage**: 95.45%



## bike-rs/bike-temporal/src/workflows/prepare_heatmap&#46;rs

```
  23             let publication = options::run(ctx, DerivedActivities::prepare_heatmap, request, options).await?;
  24             input.complete(batch, publication);
  25             if debounce::should_finish(ctx, &input, |s| &s.changes) {
  26                 break;
! 27             }
  28         }
  29         Ok(())
  30     }
```


---



## bike-rs/bike-temporal/src/workflows/rebuild_segment_analytics&#46;rs

```
  23             let publication = options::run(ctx, DerivedActivities::rebuild_segment_analytics, request, options).await?;
  24             input.complete(batch, publication);
  25             if debounce::should_finish(ctx, &input, |s| &s.changes) {
  26                 break;
! 27             }
  28         }
  29         Ok(())
  30     }
```


---


