# Diff Coverage
## Diff: 02b6c648340c26af077702f775a062a30c5c54dc...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/bin/e2e-fixtures&#46;rs (91.1%): Missing lines 184-187,199-201,203
- bike-rs/api/src/segment_support&#46;rs (0.0%): Missing lines 36,42,44-52

## Summary

- **Total**: 101 lines
- **Missing**: 19 lines
- **Coverage**: 81.19%



## bike-rs/api/src/bin/e2e-fixtures&#46;rs

```
  180     let mut input = String::new();
  181     std::io::stdin().read_to_string(&mut input)?;
  182     let mut dataset: Dataset = serde_json::from_str(&input)?;
  183     let matching = dataset.segment_matching.take();
! 184     let pairs = std::mem::take(&mut dataset.reconcile_pairs);
! 185     let recordings = std::mem::take(&mut dataset.prepare_recordings);
! 186     let acceptance = matching.is_some() || !pairs.is_empty() || !recordings.is_empty();
! 187     if staging && acceptance {
  188         return Err("Matching acceptance requires a disposable E2E project".into());
  189     }
  190     let db = bike_core::db::connect_database(&std::env::var("DATABASE_URL")?).await?;
  191     if staging && staging_already_seeded(&db, &dataset).await? {
```


---


```
  195         println!("Existing staging fixtures preserved");
  196         return Ok(());
  197     }
  198     seed(&db, dataset).await?;
! 199     let objects = bike_core::object_store::ObjectStore::filesystem(
! 200         &bike_core::config::Config::get().uploads_dir,
! 201         bike_core::object_store::ObjectOwner::ActivityImports,
  202     );
! 203     prepare_matching(&db, &objects, recordings, pairs).await?;
  204     if let Some(input) = matching {
  205         println!("{}", serde_json::to_string(&segment_matching::run(&db, input).await?)?);
  206     }
  207     Ok(())
```


---



## bike-rs/api/src/segment_support&#46;rs

```
  32 }
  33 
  34 pub async fn replace_segment_efforts_for_segment<C>(
  35     db: &C,
! 36     objects: &bike_core::object_store::ObjectStore,
  37     user_id: i32,
  38     segment_id: i32,
  39     segment_route_points: &[ActivityRoutePoint],
  40 ) -> Result<Vec<i32>, AppError>
```


---


```
  38     segment_id: i32,
  39     segment_route_points: &[ActivityRoutePoint],
  40 ) -> Result<Vec<i32>, AppError>
  41 where
! 42     C: ConnectionTrait + TransactionTrait,
  43 {
! 44     bike_core::segment_support::replace_segment_efforts_for_segment(
! 45         db,
! 46         objects,
! 47         user_id,
! 48         segment_id,
! 49         segment_route_points,
! 50     )
! 51     .await
! 52     .map_err(|error| AppError::internal(error.message))
  53 }
```


---


