//! Retain the ZIP, extract one page, and let the workflow process each file.
use super::*;
use crate::bike_tasks::{ArchiveCursor, ArchivePage, Priority};

pub async fn prepare(db: &DatabaseConnection, uploads_dir: &str, job_id: i32) -> Result<ArchiveCursor, AppError> {
    let job = activity_archive_import_jobs::Entity::find_for_batch(db, job_id)
        .await?
        .ok_or_else(|| AppError::not_found("Archive job not found"))?;
    let archive_path = Path::new(uploads_dir).join("archives").join(format!("{}.zip", job.id));
    if !archive_path.exists() {
        tokio::fs::create_dir_all(archive_path.parent().expect("archive directory")).await?;
        if Path::new(&job.archive_url).is_absolute() {
            tokio::fs::copy(&job.archive_url, &archive_path).await?;
        } else {
            let downloaded = download_archive_from_url(uploads_dir, &job.archive_url).await?;
            tokio::fs::rename(downloaded.archive_path, &archive_path).await?;
        }
    }
    mark_activity_archive_import_job_running(db, &job).await?;
    Ok(ArchiveCursor {
        job_id,
        archive_path: archive_path.to_string_lossy().into_owned(),
        offset: 0,
    })
}

pub async fn page(db: &DatabaseConnection, uploads_dir: &str, cursor: ArchiveCursor) -> Result<ArchivePage, AppError> {
    let job = activity_archive_import_jobs::Entity::find_for_batch(db, cursor.job_id)
        .await?
        .ok_or_else(|| AppError::not_found("Archive job not found"))?;
    let path = Path::new(&cursor.archive_path);
    let scan = scan_archive_entries(path)?;
    let entries = sorted_supported_archive_entries(&scan);
    let mut sources = Vec::new();
    for entry in entries.iter().skip(cursor.offset).take(16) {
        let import = retain_entry(db, uploads_dir, &job, path, entry).await?;
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
    uploads_dir: &str,
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
            uploads_dir,
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
        let archive = super::super::tests::write_test_archive(&entries);
        let cursor = ArchiveCursor {
            job_id: fixtures::archive_import_job().id,
            archive_path: archive.to_string_lossy().into_owned(),
            offset: 0,
        };
        let first = page(&page_database(16), "/unused", cursor.clone()).await.unwrap();
        assert_eq!(first.sources.len(), 16);
        assert!(!first.done);
        let retried = page(&page_database(16), "/unused", cursor.clone()).await.unwrap();
        assert_eq!(first, retried);
        let last = page(
            &page_database(1),
            "/unused",
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
        tokio::fs::remove_file(archive).await.unwrap();
    }

    #[tokio::test]
    async fn preparing_a_local_archive_retains_it_across_retries() {
        let archive = super::super::tests::write_test_archive(&[("ride.gpx", b"fixture source")]);
        let directory = tempfile::tempdir().unwrap();
        let job = activity_archive_import_jobs::Model {
            archive_url: archive.to_string_lossy().into_owned(),
            ..fixtures::archive_import_job()
        };
        for _ in 0..2 {
            let db = fixtures::postgres_mock()
                .append_query_results([vec![job.clone()], vec![job.clone()]])
                .into_connection();
            let cursor = prepare(&db, directory.path().to_str().unwrap(), job.id).await.unwrap();
            assert_eq!(
                scan_archive_entries(Path::new(&cursor.archive_path))
                    .unwrap()
                    .supported_entry_count,
                1
            );
        }
        assert!(archive.exists());
        tokio::fs::remove_file(archive).await.unwrap();
    }

    #[tokio::test]
    async fn already_retained_provider_sources_stay_on_provider_workflows() {
        let archive = super::super::tests::write_test_archive(&[("ride.gpx", b"retained source")]);
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
        let result = page(
            &db,
            "/unused",
            ArchiveCursor {
                job_id: fixtures::archive_import_job().id,
                archive_path: archive.to_string_lossy().into_owned(),
                offset: 0,
            },
        )
        .await
        .unwrap();
        assert!(result.sources.is_empty());
        assert!(result.done);
        assert_eq!(result.next_offset, 1);
        tokio::fs::remove_file(archive).await.unwrap();
    }
}
