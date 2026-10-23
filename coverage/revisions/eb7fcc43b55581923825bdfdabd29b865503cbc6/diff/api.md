# Diff Coverage
## Diff: 44d6d41bb1521ce763a727d6d14e99fbda6d379f...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/bin/e2e-fixtures&#46;rs (0.0%): Missing lines 80-85,92-94
- bike-rs/api/src/controllers/mod&#46;rs (100%)
- bike-rs/api/src/controllers/segment_overall_achievements&#46;rs (100%)

## Summary

- **Total**: 110 lines
- **Missing**: 9 lines
- **Coverage**: 91.82%



## bike-rs/api/src/bin/e2e-fixtures&#46;rs

```
  76         .iter()
  77         .filter(|record| record.entity == "activities")
  78         .filter_map(|record| record.values.get("id")?.as_i64()?.try_into().ok())
  79         .collect();
! 80     let segment_ids: Vec<i32> = dataset
! 81         .records
! 82         .iter()
! 83         .filter(|record| record.entity == "segments")
! 84         .filter_map(|record| record.values.get("id")?.as_i64()?.try_into().ok())
! 85         .collect();
  86     let tx = db.begin().await?;
  87     for record in dataset.records {
  88         store_record(&tx, record).await?;
  89     }
```


---


```
  88         store_record(&tx, record).await?;
  89     }
  90     tx.commit().await?;
  91     prepare_climbs(db, activity_ids).await?;
! 92     for id in segment_ids {
! 93         bike_core::personal_achievements::rebuild(db, id).await?;
! 94         bike_core::overall_achievements::rebuild(db, id).await?;
  95     }
  96     if dataset.prepare_heatmaps {
  97         // Reuse the owning projection builder to create persisted result data;
  98         // browser tests do not run a worker or wait for asynchronous jobs.
```


---


