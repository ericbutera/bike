# Diff Coverage
## Diff: cd46e69f4c2c7148a0dd0a077a522ebe66bb82ae...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/entities/activities&#46;rs (99.2%): Missing lines 106

## Summary

- **Total**: 129 lines
- **Missing**: 1 line
- **Coverage**: 99.22%



## bike-rs/bike-core/src/entities/activities&#46;rs

```
  102                         .as_deref()
  103                         .filter(|value| !value.is_empty())
  104                         .map(|sport| {
  105                             crate::activity_sport::activity_sport_from_stored(sport).map_or_else(
! 106                                 || Column::Sport.eq(sport),
  107                                 |sport| Column::Sport.is_in(sport.stored_values().iter().copied()),
  108                             )
  109                         }),
  110                 ),
```


---


