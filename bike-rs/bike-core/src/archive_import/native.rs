//! Retain the ZIP, extract one page, and let the workflow process each file.
use super::*;
use crate::bike_tasks::{ArchiveCursor, ArchivePage, Priority};
use crate::object_store::{ObjectOwner, ObjectStore};

pub async fn prepare(db: &DatabaseConnection, objects: &ObjectStore, job_id: i32) -> Result<ArchiveCursor, AppError> {
    let job = activity_archive_import_jobs::Entity::find_for_batch(db, job_id)
        .await?
        .ok_or_else(|| AppError::not_found("Archive job not found"))?;
    let archives = objects.for_owner(ObjectOwner::Archives);
    let archive_path = format!("archives/{}.zip", job.id);
    if !archives.contains(&archive_path).await? {
        if Path::new(&job.archive_url).is_absolute() {
            archives.retain_file(&archive_path, Path::new(&job.archive_url)).await?;
        } else {
            let downloaded = download_archive_from_url(&archives, &job.archive_url).await?;
            archives
                .retain_file(&archive_path, downloaded.archive_path.path())
                .await?;
        }
    }
    mark_activity_archive_import_job_running(db, &job).await?;
    Ok(ArchiveCursor {
        job_id,
        archive_path,
        offset: 0,
    })
}

pub async fn page(
    db: &DatabaseConnection,
    objects: &ObjectStore,
    cursor: ArchiveCursor,
) -> Result<ArchivePage, AppError> {
    let job = activity_archive_import_jobs::Entity::find_for_batch(db, cursor.job_id)
        .await?
        .ok_or_else(|| AppError::not_found("Archive job not found"))?;
    if cursor.archive_path != format!("archives/{}.zip", job.id) {
        return Err(AppError::bad_request("Archive key does not belong to this job"));
    }
    let archive = objects
        .for_owner(ObjectOwner::Archives)
        .materialize(&cursor.archive_path)
        .await?;
    let path = archive.path();
    let scan = scan_archive_entries(path)?;
    let entries = sorted_supported_archive_entries(&scan);
    let mut sources = Vec::new();
    for entry in entries.iter().skip(cursor.offset).take(16) {
        let import = retain_entry(db, objects, &job, path, entry).await?;
        if import.source == crate::entities::activity_imports::STRAVA_SOURCE {
            continue;
        }
        crate::entities::activity_imports::Entity::attach_archive_job(db, job.user_id, import.id, job.id).await?;
        sources.push(
            crate::activity_reprocessing::import_source(
                db,
                job.user_id,
                import.id,
                format!("archive-{}-import-{}", job.id, import.id),
                Priority::Background,
            )
            .await?,
        );
    }
    let next_offset = (cursor.offset + 16).min(entries.len());
    activity_archive_import_jobs::Entity::store_progress(
        db,
        job.user_id,
        job.id,
        &ActivityArchiveImportResponse {
            source: job.archive_url,
            total_entries: scan.total_entries,
            supported_entry_count: scan.supported_entry_count,
            skipped_unsupported_count: scan.skipped_unsupported_count,
            imported_count: 0,
            duplicate_count: 0,
            failed_count: 0,
            error_samples: Vec::new(),
        },
    )
    .await?;
    Ok(ArchivePage {
        sources,
        next_offset,
        done: next_offset >= entries.len(),
    })
}

async fn retain_entry(
    db: &DatabaseConnection,
    objects: &ObjectStore,
    job: &activity_archive_import_jobs::Model,
    path: &Path,
    entry: &IndexedArchiveActivityEntry,
) -> Result<crate::entities::activity_imports::Model, AppError> {
    let bytes = read_archive_entry_bytes(path, &entry.source)
        .map_err(|error| AppError::bad_request(format!("{}: {}", entry.entry_name, error.message)))?;
    let bytes = maybe_decode_archive_entry(&entry.activity_entry, bytes).map_err(AppError::bad_request)?;
    if let Some(import) = crate::activity_import_pipeline::find_stored_activity_import(db, job.user_id, &bytes).await? {
        return Ok(import);
    }
    store_activity_upload_import_with_artifacts(
        db,
        StoreActivityUploadImportRequest {
            objects,
            user_storage_key: &job.user_storage_key,
            user_id: job.user_id,
            upload: ActivityUploadPayload {
                original_filename: entry.activity_entry.original_filename.clone(),
                format: entry.activity_entry.format.clone(),
                mime_type: None,
                source_correlation_id: None,
                bytes,
            },
            primary_artifact_kind: "original",
            primary_source_quality: crate::activity_import_pipeline::original_source_quality_for_format(
                &entry.activity_entry.format,
            ),
            additional_artifacts: Vec::new(),
            source: "archive_url_import",
        },
    )
    .await
}

pub async fn finish(db: &DatabaseConnection, job_id: i32) -> Result<(), AppError> {
    let job = activity_archive_import_jobs::Entity::find_for_batch(db, job_id)
        .await?
        .ok_or_else(|| AppError::not_found("Archive job not found"))?;
    let statuses = crate::entities::activity_imports::Entity::archive_statuses(db, job_id).await?;
    let imported = statuses.iter().filter(|status| status.as_str() == "processed").count() as i32;
    let duplicate = statuses.iter().filter(|status| status.as_str() == "duplicate").count() as i32;
    mark_activity_archive_import_job_finished(
        db,
        &job,
        &ActivityArchiveImportResponse {
            source: job.archive_url.clone(),
            total_entries: job.total_entries,
            supported_entry_count: job.supported_entry_count,
            skipped_unsupported_count: job.skipped_unsupported_count,
            imported_count: imported,
            duplicate_count: duplicate,
            failed_count: 0,
            error_samples: Vec::new(),
        },
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;
    use ::object_store::ObjectStore as _;
    use futures::TryStreamExt;

    async fn retained_archive(entries: &[(&str, &[u8])]) -> (ObjectStore, ArchiveCursor) {
        let source = super::super::tests::write_test_archive(entries);
        let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
        let cursor = ArchiveCursor {
            job_id: fixtures::archive_import_job().id,
            archive_path: format!("archives/{}.zip", fixtures::archive_import_job().id),
            offset: 0,
        };
        objects
            .for_owner(ObjectOwner::Archives)
            .retain_file(&cursor.archive_path, &source)
            .await
            .unwrap();
        tokio::fs::remove_file(source).await.unwrap();
        (objects, cursor)
    }

    fn page_database(entries: usize) -> DatabaseConnection {
        let job = fixtures::archive_import_job();
        let imported = crate::entities::activity_imports::Model {
            source: "archive_url_import".into(),
            ..fixtures::activity_import()
        };
        let mut mock = fixtures::postgres_mock().append_query_results([vec![job]]);
        for _ in 0..entries {
            mock = mock
                .append_query_results([vec![fixtures::import_artifact()]])
                .append_query_results([vec![imported.clone()]])
                .append_query_results([vec![imported.clone()]])
                .append_query_results([vec![fixtures::import_artifact()]]);
        }
        mock.append_exec_results((0..=entries).map(|_| fixtures::written()))
            .into_connection()
    }

    #[tokio::test]
    async fn archive_pages_reuse_retained_imports_and_resume_from_the_offset() {
        let names: Vec<_> = (0..17).map(|index| format!("rides/{index}.gpx")).collect();
        let entries: Vec<_> = names
            .iter()
            .map(|name| (name.as_str(), b"fixture source".as_slice()))
            .collect();
        let (objects, cursor) = retained_archive(&entries).await;
        let first = page(&page_database(16), &objects, cursor.clone()).await.unwrap();
        assert_eq!(first.sources.len(), 16);
        assert!(!first.done);
        let retried = page(&page_database(16), &objects, cursor.clone()).await.unwrap();
        assert_eq!(first, retried);
        let last = page(
            &page_database(1),
            &objects,
            ArchiveCursor {
                offset: first.next_offset,
                ..cursor
            },
        )
        .await
        .unwrap();
        assert_eq!(last.sources.len(), 1);
        assert_eq!(last.next_offset, 17);
        assert!(last.done);
    }

    #[tokio::test]
    async fn preparing_a_local_archive_retains_it_across_retries() {
        let archive = super::super::tests::write_test_archive(&[("ride.gpx", b"fixture source")]);
        let (objects, backend) = fixtures::object_storage(ObjectOwner::ActivityImports);
        let job = activity_archive_import_jobs::Model {
            archive_url: archive.to_string_lossy().into_owned(),
            ..fixtures::archive_import_job()
        };
        for attempt in 0..2 {
            let db = fixtures::postgres_mock()
                .append_query_results([vec![job.clone()], vec![job.clone()]])
                .into_connection();
            let restarted = ObjectStore::new(backend.clone(), ObjectOwner::ActivityImports);
            let cursor = prepare(&db, &restarted, job.id).await.unwrap();
            assert_eq!(cursor.archive_path, format!("archives/{}.zip", job.id));
            let staged = objects
                .for_owner(ObjectOwner::Archives)
                .materialize(&cursor.archive_path)
                .await
                .unwrap();
            assert_eq!(scan_archive_entries(staged.path()).unwrap().supported_entry_count, 1);
            if attempt == 0 {
                tokio::fs::remove_file(&archive).await.unwrap();
            }
        }
        assert!(!archive.exists());
    }

    #[tokio::test]
    async fn already_retained_provider_sources_stay_on_provider_workflows() {
        let (objects, cursor) = retained_archive(&[("ride.gpx", b"retained source")]).await;
        let imported = crate::entities::activity_imports::Model {
            source: "strava_sync".into(),
            ..fixtures::activity_import()
        };
        let db = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::archive_import_job()]])
            .append_query_results([vec![fixtures::import_artifact()]])
            .append_query_results([vec![imported]])
            .append_exec_results([fixtures::written()])
            .into_connection();
        let result = page(&db, &objects, cursor).await.unwrap();
        assert!(result.sources.is_empty());
        assert!(result.done);
        assert_eq!(result.next_offset, 1);
    }

    #[tokio::test]
    async fn archive_pages_reject_keys_from_another_job_or_the_worker_filesystem() {
        let (objects, cursor) = retained_archive(&[("ride.gpx", b"source")]).await;
        for key in ["archives/999.zip", "/uploads/archives/20.zip", "../archives/20.zip"] {
            let error = page(
                &page_database(0),
                &objects,
                ArchiveCursor {
                    archive_path: key.into(),
                    ..cursor.clone()
                },
            )
            .await
            .unwrap_err();
            assert_eq!(error.status.as_u16(), 400);
        }
    }

    #[tokio::test]
    async fn new_archive_entries_retain_source_bytes_on_the_injected_import_backend() {
        let upload = fixtures::activity_upload();
        let staged = super::super::tests::write_test_archive(&[("ride.gpx", &upload.bytes)]);
        let (objects, backend) = fixtures::object_storage(ObjectOwner::ActivityImports);
        let scan = scan_archive_entries(&staged).unwrap();
        let db = fixtures::postgres_mock()
            .append_query_results([Vec::<crate::entities::activity_import_artifacts::Model>::new()])
            .append_query_results([vec![fixtures::activity_import()]])
            .append_query_results([vec![fixtures::import_artifact()]])
            .append_query_results([vec![fixtures::integration_event()]])
            .into_connection();
        retain_entry(
            &db,
            &objects,
            &fixtures::archive_import_job(),
            &staged,
            &scan.supported_entries[0],
        )
        .await
        .unwrap();
        tokio::fs::remove_file(staged).await.unwrap();
        let keys: Vec<_> = backend.list(None).try_collect().await.unwrap();
        let key = keys
            .iter()
            .find(|object| object.location.as_ref().starts_with("activity-imports/"))
            .unwrap();
        assert!(key.location.as_ref().ends_with(".gpx"));
        assert_eq!(
            objects.read_bytes(key.location.as_ref()).await.unwrap().unwrap(),
            upload.bytes
        );
        assert!(format!("{:?}", db.into_transaction_log()).contains(key.location.as_ref()));
    }
}
