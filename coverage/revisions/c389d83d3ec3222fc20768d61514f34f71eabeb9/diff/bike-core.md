# Diff Coverage
## Diff: 2888231a198b4f25c1d95818ff2b35341e5dd1bf...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (98.2%): Missing lines 1619,1629
- bike-rs/bike-core/src/activity_lifecycle&#46;rs (100%)
- bike-rs/bike-core/src/analytics&#46;rs (100%)
- bike-rs/bike-core/src/archive_import&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/batches&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities&#46;rs (100%)
- bike-rs/bike-core/src/entities/integration_events&#46;rs (100%)
- bike-rs/bike-core/src/strava/connection&#46;rs (100%)
- bike-rs/bike-core/src/test_support/defaults&#46;rs (100%)
- bike-rs/bike-core/src/test_support/mod&#46;rs (100%)
- bike-rs/bike-core/src/test_support/persistence&#46;rs (100%)
- bike-rs/bike-core/src/test_support/schema&#46;rs (83.3%): Missing lines 15
- bike-rs/bike-core/src/timestamps&#46;rs (100%)

## Summary

- **Total**: 387 lines
- **Missing**: 3 lines
- **Coverage**: 99.22%



## bike-rs/bike-core/src/activity_import_pipeline&#46;rs

```
  1615                 attempts: Set(
  1616                     if status == background_tasks::TaskStatus::Processing.as_str() {
  1617                         1
  1618                     } else {
! 1619                         0
  1620                     },
  1621                 ),
  1622                 max_attempts: Set(1),
  1623                 created_at: Set(updated_at),
```


---


```
  1625                 started_at: Set(
  1626                     if status == background_tasks::TaskStatus::Processing.as_str() {
  1627                         Some(updated_at)
  1628                     } else {
! 1629                         None
  1630                     },
  1631                 ),
  1632                 ..test_support::new_background_task()
  1633             },
```


---



## bike-rs/bike-core/src/test_support/schema&#46;rs

```
  11     // SQLite compares timestamp text; SQLx binds UTC values as RFC 3339.
  12     let clock = if db.get_database_backend() == DbBackend::Sqlite {
  13         Expr::cust("strftime('%Y-%m-%dT%H:%M:%S+00:00', 'now')")
  14     } else {
! 15         Expr::current_timestamp()
  16     };
  17     let mut table = Table::create();
  18     table.table(entity);
  19     for column in source.get_columns() {
```


---


