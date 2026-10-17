# Diff Coverage
## Diff: 3b8291e82ba259a28a05e4b3dc13ddb9dd759efb...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (95.7%): Missing lines 301,305,355
- bike-rs/bike-core/src/activity_lifecycle&#46;rs (100%)
- bike-rs/bike-core/src/archive_import&#46;rs (86.1%): Missing lines 222-225,230
- bike-rs/bike-core/src/archive_import/native&#46;rs (96.3%): Missing lines 16-19
- bike-rs/bike-core/src/heatmaps/preparation&#46;rs (100%)
- bike-rs/bike-core/src/jobs/adapter&#46;rs (100%)
- bike-rs/bike-core/src/object_store&#46;rs (100%)
- bike-rs/bike-core/src/object_store/backend&#46;rs (100%)
- bike-rs/bike-core/src/object_store/files&#46;rs (98.0%): Missing lines 20
- bike-rs/bike-core/src/test_support/imports&#46;rs (100%)

## Summary

- **Total**: 395 lines
- **Missing**: 13 lines
- **Coverage**: 96.71%



## bike-rs/bike-core/src/activity_import_pipeline&#46;rs

```
  297     upload: ActivityUploadPayload,
  298     source: &str,
  299 ) -> Result<activity_imports::Model, AppError> {
  300     let source_quality = original_source_quality_for_format(&upload.format);
! 301     let objects = ObjectStore::filesystem(uploads_dir, ObjectOwner::ActivityImports);
  302     store_activity_upload_import_with_artifacts(
  303         db,
  304         StoreActivityUploadImportRequest {
! 305             objects: &objects,
  306             user_storage_key,
  307             user_id,
  308             upload,
  309             primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
```


---


```
  351     )
  352     .await?;
  353 
  354     for artifact in additional_artifacts {
! 355         store_additional_activity_import_artifact(db, objects, user_storage_key, &import_model, artifact).await?;
  356     }
  357 
  358     record_activity_processing_event(
  359         db,
```


---



## bike-rs/bike-core/src/archive_import&#46;rs

```
  218         _ => 0,
  219     }
  220 }
  221 
! 222 pub async fn download_archive_from_url(
! 223     objects: &crate::object_store::ObjectStore,
! 224     archive_url: &str,
! 225 ) -> Result<DownloadedArchive, AppError> {
  226     let cfg = Config::get();
  227     let client = build_archive_fetch_client(cfg)?;
  228     let (final_url, response) = fetch_archive_response(&client, archive_url).await?;
  229     ensure_archive_response_size(&response, cfg.max_archive_fetch_bytes)?;
! 230     let archive_path = write_archive_response_to_temp_file(objects, response, cfg.max_archive_fetch_bytes).await?;
  231 
  232     Ok(DownloadedArchive {
  233         archive_path,
  234         final_url: final_url.to_string(),
```


---



## bike-rs/bike-core/src/archive_import/native&#46;rs

```
  12     if !archives.contains(&archive_path).await? {
  13         if Path::new(&job.archive_url).is_absolute() {
  14             archives.retain_file(&archive_path, Path::new(&job.archive_url)).await?;
  15         } else {
! 16             let downloaded = download_archive_from_url(&archives, &job.archive_url).await?;
! 17             archives
! 18                 .retain_file(&archive_path, downloaded.archive_path.path())
! 19                 .await?;
  20         }
  21     }
  22     mark_activity_archive_import_job_running(db, &job).await?;
  23     Ok(ArchiveCursor {
```


---



## bike-rs/bike-core/src/object_store/files&#46;rs

```
  16         let key = self.key(key)?;
  17         match self.backend.storage().await?.head(&key).await {
  18             Ok(_) => Ok(true),
  19             Err(::object_store::Error::NotFound { .. }) => Ok(false),
! 20             Err(error) => Err(storage_error(error)),
  21         }
  22     }
  23 
  24     /// Stream large local/downloaded inputs into storage with bounded upload memory.
```


---


