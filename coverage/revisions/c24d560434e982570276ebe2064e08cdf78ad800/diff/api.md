# Diff Coverage
## Diff: fec1a1d73f413e294b6d46fb9602c31651faeea0...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/bin/e2e-fixtures&#46;rs (97.3%): Missing lines 110

## Summary

- **Total**: 37 lines
- **Missing**: 1 line
- **Coverage**: 97.3%



## bike-rs/api/src/bin/e2e-fixtures&#46;rs

```
  106             }
  107         }
  108     }
  109     Err(sea_orm::DbErr::Custom(
! 110         "Fixture heatmap projections did not finish preparation".into(),
  111     ))
  112 }
  113 
  114 #[tokio::main]
```


---


