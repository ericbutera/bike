# Diff Coverage
## Diff: 2c6f12d080920d05f203f4f09a0756cadb705078...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/bin/e2e-fixtures&#46;rs (83.5%): Missing lines 113-117,120,122-128

## Summary

- **Total**: 79 lines
- **Missing**: 13 lines
- **Coverage**: 83.54%



## bike-rs/api/src/bin/e2e-fixtures&#46;rs

```
  109 }
  110 
  111 #[tokio::main]
  112 async fn main() -> Result<(), Box<dyn std::error::Error>> {
! 113     let staging = fixture_mode(
! 114         std::env::var("BIKE_E2E_PROJECT").ok().as_deref(),
! 115         std::env::var("BIKE_FIXTURE_TARGET").ok().as_deref(),
! 116         std::env::var("APP_ENV").ok().as_deref(),
! 117     )?;
  118     let mut input = String::new();
  119     std::io::stdin().read_to_string(&mut input)?;
! 120     let dataset: Dataset = serde_json::from_str(&input)?;
  121     let db = bike_core::db::connect_database(&std::env::var("DATABASE_URL")?).await?;
! 122     if staging && staging_already_seeded(&db, &dataset).await? {
! 123         if dataset.prepare_heatmaps {
! 124             prepare_heatmaps(&db).await?;
! 125         }
! 126         println!("Existing staging fixtures preserved");
! 127         return Ok(());
! 128     }
  129     seed(&db, dataset).await?;
  130     Ok(())
  131 }
```


---


