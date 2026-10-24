# Diff Coverage
## Diff: 7a6ca2555bb74486c3a54865baa3651a7a8d642b...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/analytics&#46;rs (100%)
- bike-rs/bike-core/src/entities/segment_efforts/ranks&#46;rs (100%)
- bike-rs/bike-core/src/entities/segment_user_summaries&#46;rs (98.5%): Missing lines 165

## Summary

- **Total**: 153 lines
- **Missing**: 1 line
- **Coverage**: 99.35%



## bike-rs/bike-core/src/entities/segment_user_summaries&#46;rs

```
  161                 let values = &statement.values.as_ref().unwrap().0;
  162                 for row in values.as_chunks::<5>().0 {
  163                     let user_id = match row[1] {
  164                         sea_orm::Value::Int(Some(id)) => id,
! 165                         _ => panic!("expected rider id"),
  166                     };
  167                     assert_eq!(
  168                         row,
  169                         &[
```


---


