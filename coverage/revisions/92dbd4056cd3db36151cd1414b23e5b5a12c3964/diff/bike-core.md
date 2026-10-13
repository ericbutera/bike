# Diff Coverage
## Diff: f73e6a378831ef2cb576d65a7fb4b8da1b08b607...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (95.5%): Missing lines 1329
- bike-rs/bike-core/src/background_jobs/entities/background_tasks&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_import_artifacts&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_imports&#46;rs (100%)
- bike-rs/bike-core/src/entities/integration_events&#46;rs (80.0%): Missing lines 38-41,43
- bike-rs/bike-core/src/entities/strava_workflows&#46;rs (100%)
- bike-rs/bike-core/src/strava_activity&#46;rs (100%)
- bike-rs/bike-core/src/strava_objects&#46;rs (100%)
- bike-rs/bike-core/src/strava_retention/files&#46;rs (92.0%): Missing lines 58-60,86,129-130,147,154,157
- bike-rs/bike-core/src/strava_retention/metrics&#46;rs (93.2%): Missing lines 60-63
- bike-rs/bike-core/src/strava_retention/mod&#46;rs (96.0%): Missing lines 19-21
- bike-rs/bike-core/src/strava_retention/protection&#46;rs (96.9%): Missing lines 102,116-117
- bike-rs/bike-core/src/strava_workflow/activity&#46;rs (55.6%): Missing lines 228,230-235,237
- bike-rs/bike-core/src/strava_workflow/inbox&#46;rs (80.0%): Missing lines 39
- bike-rs/bike-core/src/strava_workflow/mod&#46;rs (94.4%): Missing lines 156

## Summary

- **Total**: 519 lines
- **Missing**: 35 lines
- **Coverage**: 93.26%



## bike-rs/bike-core/src/activity_import_pipeline&#46;rs

```
  1325         activity_import_storage_path(request.user_storage_key, &request.upload.format, Utc::now());
  1326     let full_path = Path::new(request.uploads_dir).join(&relative);
  1327     if let Some(parent) = full_path.parent() {
  1328         tokio::fs::create_dir_all(parent).await?;
! 1329     }
  1330     tokio::fs::write(full_path, &request.upload.bytes).await?;
  1331     Ok(relative)
  1332 }
```


---



## bike-rs/bike-core/src/entities/integration_events&#46;rs

```
  34         let workflow = match db.get_database_backend() {
  35             DatabaseBackend::Sqlite => Func::cust(Alias::new("json_extract"))
  36                 .args([payload, Expr::val("$.workflow_id")])
  37                 .into(),
! 38             DatabaseBackend::MySql => Func::cust(Alias::new("JSON_UNQUOTE"))
! 39                 .arg(
! 40                     Func::cust(Alias::new("JSON_EXTRACT"))
! 41                         .args([payload, Expr::val("$.workflow_id")]),
  42                 )
! 43                 .into(),
  44             _ => PgExpr::cast_json_field(payload, "workflow_id"),
  45         };
  46         let mut deleted = 0;
  47         for batch in ids.chunks(500) {
```


---



## bike-rs/bike-core/src/strava_retention/files&#46;rs

```
  54     for source in sources.difference(&paths) {
  55         let path = store.path(source)?;
  56         let metadata = fs::symlink_metadata(path).await?;
  57         if !metadata.is_file() {
! 58             return Err(WorkflowError::internal(
! 59                 "Strava rebuild source must be a regular file",
! 60             ));
  61         }
  62         files.push(StoredFile {
  63             path: source.clone(),
  64             bytes: metadata.len(),
```


---


```
  82         let path = root.join(&relative);
  83         let metadata = match fs::symlink_metadata(&path).await {
  84             Ok(metadata) => metadata,
  85             Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
! 86             Err(error) => return Err(error.into()),
  87         };
  88         if metadata.is_dir() {
  89             let mut entries = fs::read_dir(path).await?;
  90             while let Some(entry) = entries.next_entry().await? {
```


---


```
  125                 metrics::pruned(file.bytes);
  126                 let namespace = store.path(file.path.split('/').next().unwrap_or("strava"))?;
  127                 remove_empty_parents(path.parent(), &namespace).await?;
  128             }
! 129             Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
! 130             Err(error) => return Err(error.into()),
  131         }
  132     }
  133     Ok(result)
  134 }
```


---


```
  143         }
  144         match fs::remove_dir(path).await {
  145             Ok(()) => directory = path.parent(),
  146             Err(error)
! 147                 if matches!(
  148                     error.kind(),
  149                     std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
  150                 ) =>
  151             {
```


---


```
  150                 ) =>
  151             {
  152                 return Ok(())
  153             }
! 154             Err(error) => return Err(error.into()),
  155         }
  156     }
! 157     Ok(())
  158 }
```


---



## bike-rs/bike-core/src/strava_retention/metrics&#46;rs

```
  56     PRUNED_FILES.inc();
  57     PRUNED_BYTES.inc_by(bytes);
  58 }
  59 
! 60 pub(super) fn init() {
! 61     Lazy::force(&PRUNED_FILES);
! 62     Lazy::force(&PRUNED_BYTES);
! 63 }
  64 
  65 pub(super) fn publish(
  66     inventory: &[StoredFile],
  67     sources: &BTreeSet<String>,
```


---



## bike-rs/bike-core/src/strava_retention/mod&#46;rs

```
  15 use std::collections::BTreeSet;
  16 
  17 pub const TTL_DAYS: i64 = 30;
  18 
! 19 pub fn init_metrics() {
! 20     metrics::init();
! 21 }
  22 
  23 #[derive(Debug, Default)]
  24 pub struct RetentionResult {
  25     pub files: u64,
```


---



## bike-rs/bike-core/src/strava_retention/protection&#46;rs

```
   98             self.directories
   99                 .insert(parent.to_string_lossy().into_owned());
  100             if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
  101                 references(&value, &mut objects)?;
! 102             }
  103         }
  104         Ok(())
  105     }
  106 }
```


---


```
  112                 && fields.contains_key("sha256")
  113                 && fields.contains_key("bytes") =>
  114         {
  115             output.push(serde_json::from_value(value.clone()).map_err(|error| {
! 116                 WorkflowError::internal(format!("Invalid saved object: {error}"))
! 117             })?);
  118         }
  119         Value::Object(fields) => {
  120             for value in fields.values() {
  121                 references(value, output)?;
```


---



## bike-rs/bike-core/src/strava_workflow/activity&#46;rs

```
  224                 sha256: artifact.checksum_sha256,
  225                 bytes: usize::try_from(artifact.size_bytes)
  226                     .map_err(|_| WorkflowError::internal("Invalid Strava source size"))?,
  227             }
! 228         } else if import.format == "json" {
  229             // Pre-artifact imports remain readable without inventing provenance.
! 230             let bytes = tokio::fs::read(
! 231                 std::path::Path::new(&crate::config::Config::get().uploads_dir)
! 232                     .join(import.storage_path),
! 233             )
! 234             .await?;
! 235             return serde_json::from_slice(&bytes).map(Some).map_err(json_error);
  236         } else {
! 237             return Ok(None);
  238         };
  239         let bytes = self.store.read(&object).await?;
  240         serde_json::from_slice(&bytes).map(Some).map_err(json_error)
  241     }
```


---



## bike-rs/bike-core/src/strava_workflow/inbox&#46;rs

```
  35     let txn = db.begin().await?;
  36     if db.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
  37         txn.execute_unprepared("SELECT pg_advisory_xact_lock_shared(7393, 0)")
  38             .await?;
! 39     }
  40     let store = ObjectStore::new(&crate::config::Config::get().uploads_dir);
  41     let raw = store
  42         .write(
  43             &format!(
```


---



## bike-rs/bike-core/src/strava_workflow/mod&#46;rs

```
  152             .execute_unprepared(&format!(
  153                 "SELECT pg_advisory_xact_lock(7391, {connection_id})"
  154             ))
  155             .await?;
! 156     }
  157     Ok(lease)
  158 }
  159 
  160 async fn execute_owned(
```


---


