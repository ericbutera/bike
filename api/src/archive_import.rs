use crate::activity_import_lock::{
    acquire_user_activity_import_lock, mark_user_activity_import_lock_stage,
    release_user_activity_import_lock, ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
    ACTIVITY_IMPORT_LOCK_STAGE_QUEUED, ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::activity_import_pipeline::{
    finalize_activity_import_batch, persist_activity_upload, ActivityUploadDeduplication,
    ActivityUploadPayload, PersistActivityUploadOutcome,
};
use crate::app_error::AppError;
use crate::config::Config;
use crate::entities::activity_archive_import_jobs;
use crate::tasks::TaskQueue;
use crate::training_profile::load_training_profile;
use chrono::Utc;
use reqwest::{header, redirect::Policy, Client, Url};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::Serialize;
use std::env;
use std::io::{Cursor, Read, Seek};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::lookup_host;
use utoipa::ToSchema;
use uuid::Uuid;
use zip::ZipArchive;

const MAX_ARCHIVE_REDIRECTS: usize = 5;
pub const ACTIVITY_ARCHIVE_IMPORT_STATUS_QUEUED: &str = "queued";
pub const ACTIVITY_ARCHIVE_IMPORT_STATUS_RUNNING: &str = "running";
pub const ACTIVITY_ARCHIVE_IMPORT_STATUS_SUCCEEDED: &str = "succeeded";
pub const ACTIVITY_ARCHIVE_IMPORT_STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ActivityArchiveImportResponse {
    pub source: String,
    pub total_entries: i32,
    pub supported_entry_count: i32,
    pub imported_count: i32,
    pub duplicate_count: i32,
    pub skipped_unsupported_count: i32,
    pub failed_count: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub error_samples: Vec<String>,
}

pub struct DownloadedArchive {
    pub archive_path: PathBuf,
    pub final_url: String,
}

pub async fn enqueue_activity_archive_import_job(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
    user_storage_key: &str,
    archive_url: String,
) -> Result<activity_archive_import_jobs::Model, AppError> {
    acquire_user_activity_import_lock(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
        ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    )
    .await?;

    let job = activity_archive_import_jobs::ActiveModel {
        user_id: Set(user_id),
        user_storage_key: Set(user_storage_key.to_string()),
        archive_url: Set(archive_url),
        status: Set(ACTIVITY_ARCHIVE_IMPORT_STATUS_QUEUED.to_string()),
        total_entries: Set(0),
        supported_entry_count: Set(0),
        imported_count: Set(0),
        duplicate_count: Set(0),
        skipped_unsupported_count: Set(0),
        failed_count: Set(0),
        ..Default::default()
    }
    .insert(db)
    .await;

    let job = match job {
        Ok(job) => job,
        Err(error) => {
            release_user_activity_import_lock(
                db,
                user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
            )
            .await?;
            return Err(AppError::from(error));
        }
    };

    if let Err(message) = tasks.archive_activity_import(job.id).await {
        release_user_activity_import_lock(db, user_id, ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT)
            .await?;
        let failed_job = mark_activity_archive_import_job_failed(
            db,
            &job,
            None,
            format!("Failed to enqueue archive import task: {message}"),
        )
        .await?;
        return Err(AppError::internal(format!(
            "Failed to enqueue archive import task {} for user {}: {}",
            failed_job.id, failed_job.user_id, message
        )));
    }

    Ok(job)
}

pub async fn process_activity_archive_import_job(
    db: &DatabaseConnection,
    uploads_dir: &str,
    job_id: i32,
) -> Result<(), AppError> {
    let job = activity_archive_import_jobs::Entity::find_by_id(job_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::internal(format!("Archive import job {job_id} was not found")))?;

    mark_user_activity_import_lock_stage(
        db,
        job.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let running_job = mark_activity_archive_import_job_running(db, &job).await?;
    let downloaded = match download_archive_from_url(uploads_dir, &running_job.archive_url).await {
        Ok(downloaded) => downloaded,
        Err(error) => {
            mark_activity_archive_import_job_failed(db, &running_job, None, error.message.clone())
                .await?;
            release_user_activity_import_lock(
                db,
                running_job.user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
            )
            .await?;
            return Err(error);
        }
    };

    let result = import_activity_archive_from_path(
        db,
        &TaskQueue::new(db.clone()),
        uploads_dir,
        &running_job.user_storage_key,
        running_job.user_id,
        "archive_url_import",
        downloaded.final_url.clone(),
        &downloaded.archive_path,
    )
    .await;

    let _ = tokio::fs::remove_file(&downloaded.archive_path).await;

    match result {
        Ok(summary) => {
            mark_activity_archive_import_job_succeeded(db, &running_job, &summary).await?;
            release_user_activity_import_lock(
                db,
                running_job.user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
            )
            .await?;
            Ok(())
        }
        Err(error) => {
            mark_activity_archive_import_job_failed(
                db,
                &running_job,
                Some(downloaded.final_url),
                error.message.clone(),
            )
            .await?;
            release_user_activity_import_lock(
                db,
                running_job.user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
            )
            .await?;
            Err(error)
        }
    }
}

pub fn normalize_archive_url(raw: &str) -> Result<String, AppError> {
    parse_archive_url(raw).map(|url| url.to_string())
}

pub fn resolve_local_archive_import_path(
    uploads_dir: &str,
    archive_path: &str,
) -> Result<PathBuf, AppError> {
    let trimmed = archive_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation_field(
            "archive_path",
            "Archive path is required",
        ));
    }

    let requested = PathBuf::from(trimmed);
    let resolved = if requested.is_absolute() {
        requested
    } else {
        Path::new(uploads_dir).join(requested)
    };

    let extension = resolved
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    if extension.as_deref() != Some("zip") {
        return Err(AppError::validation_field(
            "archive_path",
            "Archive path must point to a .zip file",
        ));
    }

    if !resolved.exists() {
        return Err(AppError::not_found(format!(
            "Archive not found: {}",
            resolved.display()
        )));
    }

    if !resolved.is_file() {
        return Err(AppError::validation_field(
            "archive_path",
            "Archive path must point to a file",
        ));
    }

    Ok(resolved)
}

pub fn decode_error_samples(raw: Option<&str>) -> Vec<String> {
    raw.and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default()
}

pub fn is_archive_import_terminal_status(status: &str) -> bool {
    matches!(
        status,
        ACTIVITY_ARCHIVE_IMPORT_STATUS_SUCCEEDED | ACTIVITY_ARCHIVE_IMPORT_STATUS_FAILED
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArchiveActivityEntry {
    original_filename: String,
    format: String,
    gzip_wrapped: bool,
}

#[derive(Debug, Clone)]
struct IndexedArchiveActivityEntry {
    source: ArchiveEntrySource,
    entry_name: String,
    activity_entry: ArchiveActivityEntry,
}

#[derive(Debug, Clone)]
enum ArchiveEntrySource {
    Root {
        index: usize,
    },
    NestedZip {
        outer_index: usize,
        outer_name: String,
        inner_index: usize,
        inner_name: String,
    },
}

#[derive(Debug, Clone)]
struct ArchiveScanResult {
    total_entries: i32,
    supported_entry_count: i32,
    skipped_unsupported_count: i32,
    supported_entries: Vec<IndexedArchiveActivityEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveScanMode {
    Generic,
    StravaExport,
}

pub async fn import_activity_archive_from_path(
    db: &sea_orm::DatabaseConnection,
    tasks: &TaskQueue,
    uploads_dir: &str,
    user_storage_key: &str,
    user_id: i32,
    activity_source: &str,
    display_source: String,
    archive_path: &Path,
) -> Result<ActivityArchiveImportResponse, AppError> {
    let scan = scan_archive_entries(archive_path)?;
    let training_profile = load_training_profile(db, user_id).await?;
    let mut imported_count = 0i32;
    let mut duplicate_count = 0i32;
    let mut affected_segment_ids = Vec::new();
    let mut fitness_dirty_from_day: Option<chrono::NaiveDate> = None;
    let mut error_samples = Vec::new();

    for indexed_entry in &scan.supported_entries {
        let bytes = match read_archive_entry_bytes(archive_path, &indexed_entry.source) {
            Ok(value) => value,
            Err(error) => {
                error_samples.push(format!(
                    "{}: failed to read archive entry: {}",
                    indexed_entry.entry_name, error.message
                ));
                continue;
            }
        };

        let bytes = match maybe_decode_archive_entry(&indexed_entry.activity_entry, bytes) {
            Ok(value) => value,
            Err(message) => {
                error_samples.push(format!("{}: {}", indexed_entry.entry_name, message));
                continue;
            }
        };

        let upload = ActivityUploadPayload {
            original_filename: indexed_entry.activity_entry.original_filename.clone(),
            format: indexed_entry.activity_entry.format.clone(),
            mime_type: None,
            source_correlation_id: None,
            bytes,
        };

        match persist_activity_upload(
            db,
            uploads_dir,
            user_storage_key,
            user_id,
            upload,
            activity_source,
            ActivityUploadDeduplication::Enabled,
            Some(&training_profile),
        )
        .await
        {
            Ok(PersistActivityUploadOutcome::Imported(persisted)) => {
                imported_count += 1;
                affected_segment_ids.extend(persisted.affected_segment_ids);
                fitness_dirty_from_day = Some(match fitness_dirty_from_day {
                    Some(current) => current.min(persisted.fitness_dirty_from_day),
                    None => persisted.fitness_dirty_from_day,
                });
            }
            Ok(PersistActivityUploadOutcome::Duplicate(_)) => {
                duplicate_count += 1;
            }
            Err(error) => {
                error_samples.push(format!("{}: {}", indexed_entry.entry_name, error.message));
            }
        }
    }

    if scan.supported_entry_count == 0 {
        return Err(AppError::validation_field(
            "archive_url",
            "Archive did not contain any supported .fit, .tcx, or .gpx files",
        ));
    }

    if imported_count > 0 {
        finalize_activity_import_batch(
            db,
            tasks,
            user_id,
            affected_segment_ids,
            fitness_dirty_from_day,
            Utc::now(),
        )
        .await?;
    }

    let failed_count = error_samples.len() as i32;
    let error_samples = error_samples.into_iter().take(10).collect::<Vec<_>>();

    Ok(ActivityArchiveImportResponse {
        source: display_source,
        total_entries: scan.total_entries,
        supported_entry_count: scan.supported_entry_count,
        imported_count,
        duplicate_count,
        skipped_unsupported_count: scan.skipped_unsupported_count,
        failed_count,
        error_samples,
    })
}

pub async fn download_archive_from_url(
    uploads_dir: &str,
    archive_url: &str,
) -> Result<DownloadedArchive, AppError> {
    let cfg = Config::get();
    let client = Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(cfg.archive_fetch_timeout_seconds))
        .build()
        .map_err(|error| {
            AppError::internal(format!("Failed to build archive fetch client: {error}"))
        })?;
    let mut current_url = parse_archive_url(archive_url)?;

    for _ in 0..=MAX_ARCHIVE_REDIRECTS {
        validate_remote_archive_url(&current_url).await?;

        let response = client
            .get(current_url.clone())
            .send()
            .await
            .map_err(|error| {
                AppError::bad_request(format!("Failed to fetch archive URL: {error}"))
            })?;

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    AppError::bad_request("Archive URL redirected without a valid Location header")
                })?;
            current_url = current_url.join(location).map_err(|error| {
                AppError::bad_request(format!("Archive URL redirect was invalid: {error}"))
            })?;
            continue;
        }

        if !response.status().is_success() {
            return Err(AppError::bad_request(format!(
                "Archive URL returned HTTP {}",
                response.status()
            )));
        }

        if response
            .content_length()
            .is_some_and(|length| length as usize > cfg.max_archive_fetch_bytes)
        {
            return Err(AppError::payload_too_large(
                "archive_url",
                format!(
                    "Archive exceeds the {} byte fetch limit",
                    cfg.max_archive_fetch_bytes
                ),
            ));
        }

        let temp_path = Path::new(uploads_dir)
            .join("archive-fetches")
            .join(format!("{}.zip", Uuid::new_v4()));

        if let Some(parent) = temp_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = tokio::fs::File::create(&temp_path).await?;
        let mut total_bytes = 0usize;
        let mut response = response;

        while let Some(chunk) = response.chunk().await.map_err(|error| {
            AppError::bad_request(format!("Failed to download archive URL: {error}"))
        })? {
            total_bytes += chunk.len();
            if total_bytes > cfg.max_archive_fetch_bytes {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(AppError::payload_too_large(
                    "archive_url",
                    format!(
                        "Archive exceeds the {} byte fetch limit",
                        cfg.max_archive_fetch_bytes
                    ),
                ));
            }
            file.write_all(&chunk).await?;
        }

        file.flush().await?;

        if total_bytes == 0 {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(AppError::validation_field(
                "archive_url",
                "Archive URL returned an empty response",
            ));
        }

        return Ok(DownloadedArchive {
            archive_path: temp_path,
            final_url: current_url.to_string(),
        });
    }

    Err(AppError::bad_request(
        "Archive URL redirected too many times",
    ))
}

fn parse_archive_url(raw: &str) -> Result<Url, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation_field(
            "archive_url",
            "Archive URL is required",
        ));
    }

    let url = Url::parse(trimmed).map_err(|error| {
        AppError::validation_field("archive_url", format!("Archive URL is invalid: {error}"))
    })?;

    // Allow http URLs for local development/self-hosted file servers when
    // the operator explicitly enables it via the `ALLOW_LOCAL_ARCHIVE_IMPORTS`
    // environment variable. Otherwise require https.
    let allow_local = env::var("ALLOW_LOCAL_ARCHIVE_IMPORTS").is_ok();
    if url.scheme() != "https" {
        // If the operator enabled local imports, accept http URLs. This
        // permits pointing at LAN or localhost file servers during development.
        if !(allow_local && url.scheme() == "http") {
            return Err(AppError::validation_field(
                "archive_url",
                "Archive URL must use https",
            ));
        }
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::validation_field(
            "archive_url",
            "Archive URL must not include embedded credentials",
        ));
    }

    if url.host_str().is_none() {
        return Err(AppError::validation_field(
            "archive_url",
            "Archive URL must include a host",
        ));
    }

    Ok(url)
}

async fn validate_remote_archive_url(url: &Url) -> Result<(), AppError> {
    let host = url.host_str().ok_or_else(|| {
        AppError::validation_field("archive_url", "Archive URL must include a host")
    })?;
    let port = url.port_or_known_default().ok_or_else(|| {
        AppError::validation_field("archive_url", "Archive URL must include a valid port")
    })?;
    // If local archive imports are allowed via env, skip the public IP check
    // so developers can point the importer at a local Caddy or other file
    // server during development. Otherwise enforce that the host resolves to
    // a public internet address.
    let allow_local = env::var("ALLOW_LOCAL_ARCHIVE_IMPORTS").is_ok();
    if allow_local {
        return Ok(());
    }

    let mut saw_address = false;

    for address in lookup_host((host, port)).await.map_err(|error| {
        AppError::bad_request(format!("Failed to resolve archive URL host: {error}"))
    })? {
        saw_address = true;
        if !is_public_ip(address.ip()) {
            return Err(AppError::validation_field(
                "archive_url",
                "Archive URL must resolve to a public internet address",
            ));
        }
    }

    if !saw_address {
        return Err(AppError::validation_field(
            "archive_url",
            "Archive URL did not resolve to any addresses",
        ));
    }

    Ok(())
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(value) => {
            !value.is_private()
                && !value.is_loopback()
                && !value.is_link_local()
                && !value.is_broadcast()
                && !value.is_documentation()
                && !value.is_unspecified()
        }
        IpAddr::V6(value) => {
            let segments = value.segments();
            let is_documentation = segments[0] == 0x2001 && segments[1] == 0x0db8;

            !value.is_loopback()
                && !value.is_multicast()
                && !value.is_unspecified()
                && !value.is_unique_local()
                && !value.is_unicast_link_local()
                && !is_documentation
        }
    }
}

fn scan_archive_entries(archive_path: &Path) -> Result<ArchiveScanResult, AppError> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| AppError::bad_request(format!("Failed to open zip archive: {error}")))?;
    let mut scan = ArchiveScanResult {
        total_entries: 0,
        supported_entry_count: 0,
        skipped_unsupported_count: 0,
        supported_entries: Vec::new(),
    };

    scan_zip_archive(&mut archive, &mut scan, None)?;
    scan.supported_entry_count = scan.supported_entries.len() as i32;

    Ok(scan)
}

fn scan_zip_archive<R>(
    archive: &mut ZipArchive<R>,
    scan: &mut ArchiveScanResult,
    outer: Option<(usize, &str)>,
) -> Result<(), AppError>
where
    R: Read + Seek,
{
    let scan_mode = detect_archive_scan_mode(archive)?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            AppError::bad_request(format!("Failed to read zip archive entry: {error}"))
        })?;

        if entry.is_dir() {
            continue;
        }

        let entry_name = entry.name().to_string();
        if archive_entry_can_be_activity(&entry_name, scan_mode) {
            if let Some(activity_entry) = resolve_archive_activity_entry(&entry_name) {
                scan.total_entries += 1;
                let (source, display_name) = match outer {
                    Some((outer_index, outer_name)) => (
                        ArchiveEntrySource::NestedZip {
                            outer_index,
                            outer_name: outer_name.to_string(),
                            inner_index: index,
                            inner_name: entry_name.clone(),
                        },
                        format!("{outer_name}::{entry_name}"),
                    ),
                    None => (ArchiveEntrySource::Root { index }, entry_name.clone()),
                };

                scan.supported_entries.push(IndexedArchiveActivityEntry {
                    source,
                    entry_name: display_name,
                    activity_entry,
                });
                continue;
            }
        }

        if outer.is_none()
            && scan_mode == ArchiveScanMode::Generic
            && is_nested_zip_entry(&entry_name)
        {
            let mut nested_bytes = Vec::new();
            entry.read_to_end(&mut nested_bytes)?;
            let mut nested_archive =
                ZipArchive::new(Cursor::new(nested_bytes)).map_err(|error| {
                    AppError::bad_request(format!("Failed to open nested zip archive: {error}"))
                })?;
            scan_zip_archive(
                &mut nested_archive,
                scan,
                Some((index, entry_name.as_str())),
            )?;
            continue;
        }

        scan.total_entries += 1;
        scan.skipped_unsupported_count += 1;
    }

    Ok(())
}

fn detect_archive_scan_mode<R>(archive: &mut ZipArchive<R>) -> Result<ArchiveScanMode, AppError>
where
    R: Read + Seek,
{
    let mut has_activities_csv = false;
    let mut has_activities_directory = false;

    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            AppError::bad_request(format!("Failed to read zip archive entry: {error}"))
        })?;

        if entry.is_dir() {
            continue;
        }

        let entry_name = entry.name();
        if is_root_archive_file(entry_name, "activities.csv") {
            has_activities_csv = true;
        }
        if has_top_level_archive_directory(entry_name, "activities") {
            has_activities_directory = true;
        }

        if has_activities_csv && has_activities_directory {
            return Ok(ArchiveScanMode::StravaExport);
        }
    }

    Ok(ArchiveScanMode::Generic)
}

fn archive_entry_can_be_activity(name: &str, scan_mode: ArchiveScanMode) -> bool {
    match scan_mode {
        ArchiveScanMode::Generic => true,
        ArchiveScanMode::StravaExport => has_top_level_archive_directory(name, "activities"),
    }
}

fn is_root_archive_file(name: &str, file_name: &str) -> bool {
    let mut components = name.split('/').filter(|value| !value.is_empty());
    matches!((components.next(), components.next()), (Some(component), None) if component.eq_ignore_ascii_case(file_name))
}

fn has_top_level_archive_directory(name: &str, directory: &str) -> bool {
    let mut components = name.split('/').filter(|value| !value.is_empty());
    matches!((components.next(), components.next()), (Some(component), Some(_)) if component.eq_ignore_ascii_case(directory))
}

fn read_archive_entry_bytes(
    archive_path: &Path,
    source: &ArchiveEntrySource,
) -> Result<Vec<u8>, AppError> {
    match source {
        ArchiveEntrySource::Root { index } => {
            let file = std::fs::File::open(archive_path)?;
            let mut archive = ZipArchive::new(file).map_err(|error| {
                AppError::bad_request(format!("Failed to open zip archive: {error}"))
            })?;
            let mut entry = archive.by_index(*index).map_err(|error| {
                AppError::bad_request(format!("Failed to read zip archive entry: {error}"))
            })?;
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            Ok(bytes)
        }
        ArchiveEntrySource::NestedZip {
            outer_index,
            outer_name,
            inner_index,
            inner_name,
        } => {
            let file = std::fs::File::open(archive_path)?;
            let mut archive = ZipArchive::new(file).map_err(|error| {
                AppError::bad_request(format!("Failed to open zip archive: {error}"))
            })?;
            let mut outer_entry = archive.by_index(*outer_index).map_err(|error| {
                AppError::bad_request(format!("Failed to read zip archive entry: {error}"))
            })?;
            let mut outer_bytes = Vec::new();
            outer_entry.read_to_end(&mut outer_bytes)?;

            let mut nested_archive =
                ZipArchive::new(Cursor::new(outer_bytes)).map_err(|error| {
                    AppError::bad_request(format!(
                        "Failed to open nested zip archive {outer_name}: {error}"
                    ))
                })?;
            let mut inner_entry = nested_archive.by_index(*inner_index).map_err(|error| {
                AppError::bad_request(format!(
                    "Failed to read nested zip archive entry {outer_name}::{inner_name}: {error}"
                ))
            })?;
            let mut bytes = Vec::new();
            inner_entry.read_to_end(&mut bytes)?;
            Ok(bytes)
        }
    }
}

fn is_nested_zip_entry(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".zip")
}

fn resolve_archive_activity_entry(name: &str) -> Option<ArchiveActivityEntry> {
    let file_name = Path::new(name).file_name()?.to_str()?.trim();
    if file_name.is_empty() {
        return None;
    }

    let lower = file_name.to_ascii_lowercase();
    for (suffix, format, gzip_wrapped) in [
        (".fit.gz", "fit", true),
        (".tcx.gz", "tcx", true),
        (".gpx.gz", "gpx", true),
        (".fit", "fit", false),
        (".tcx", "tcx", false),
        (".gpx", "gpx", false),
    ] {
        if lower.ends_with(suffix) {
            let original_filename = if gzip_wrapped {
                file_name[..file_name.len() - 3].to_string()
            } else {
                file_name.to_string()
            };

            return Some(ArchiveActivityEntry {
                original_filename,
                format: format.to_string(),
                gzip_wrapped,
            });
        }
    }

    None
}

fn maybe_decode_archive_entry(
    entry: &ArchiveActivityEntry,
    bytes: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if !entry.gzip_wrapped {
        return Ok(bytes);
    }

    let mut decoder = flate2::read::GzDecoder::new(bytes.as_slice());
    let mut decoded = Vec::new();
    decoder
        .read_to_end(&mut decoded)
        .map_err(|error| format!("failed to decode gzip-compressed activity: {error}"))?;

    Ok(decoded)
}

async fn mark_activity_archive_import_job_running(
    db: &DatabaseConnection,
    job: &activity_archive_import_jobs::Model,
) -> Result<activity_archive_import_jobs::Model, AppError> {
    activity_archive_import_jobs::ActiveModel {
        id: Set(job.id),
        status: Set(ACTIVITY_ARCHIVE_IMPORT_STATUS_RUNNING.to_string()),
        failure_message: Set(None),
        error_samples_json: Set(None),
        resolved_url: Set(None),
        total_entries: Set(0),
        supported_entry_count: Set(0),
        imported_count: Set(0),
        duplicate_count: Set(0),
        skipped_unsupported_count: Set(0),
        failed_count: Set(0),
        started_at: Set(Some(Utc::now())),
        finished_at: Set(None),
        ..Default::default()
    }
    .update(db)
    .await
    .map_err(AppError::from)
}

async fn mark_activity_archive_import_job_succeeded(
    db: &DatabaseConnection,
    job: &activity_archive_import_jobs::Model,
    summary: &ActivityArchiveImportResponse,
) -> Result<activity_archive_import_jobs::Model, AppError> {
    activity_archive_import_jobs::ActiveModel {
        id: Set(job.id),
        resolved_url: Set(Some(summary.source.clone())),
        status: Set(ACTIVITY_ARCHIVE_IMPORT_STATUS_SUCCEEDED.to_string()),
        failure_message: Set(None),
        error_samples_json: Set(Some(
            serde_json::to_string(&summary.error_samples).map_err(|error| {
                AppError::internal(format!(
                    "Failed to serialize archive import errors: {error}"
                ))
            })?,
        )),
        total_entries: Set(summary.total_entries),
        supported_entry_count: Set(summary.supported_entry_count),
        imported_count: Set(summary.imported_count),
        duplicate_count: Set(summary.duplicate_count),
        skipped_unsupported_count: Set(summary.skipped_unsupported_count),
        failed_count: Set(summary.failed_count),
        finished_at: Set(Some(Utc::now())),
        ..Default::default()
    }
    .update(db)
    .await
    .map_err(AppError::from)
}

async fn mark_activity_archive_import_job_failed(
    db: &DatabaseConnection,
    job: &activity_archive_import_jobs::Model,
    resolved_url: Option<String>,
    failure_message: String,
) -> Result<activity_archive_import_jobs::Model, AppError> {
    activity_archive_import_jobs::ActiveModel {
        id: Set(job.id),
        resolved_url: Set(resolved_url),
        status: Set(ACTIVITY_ARCHIVE_IMPORT_STATUS_FAILED.to_string()),
        failure_message: Set(Some(failure_message)),
        finished_at: Set(Some(Utc::now())),
        ..Default::default()
    }
    .update(db)
    .await
    .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{
        activities, activity_analytics, activity_imports, analytics_user_states, segment_efforts,
        segments, user_preferences,
    };
    use kaleido::background_jobs::background_tasks;
    use sea_orm::{ConnectionTrait, Database, EntityTrait, PaginatorTrait, Schema};
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn repo_data_archive_path(file_name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("data")
            .join(file_name)
    }

    fn write_test_archive(entries: &[(&str, &[u8])]) -> std::path::PathBuf {
        let archive_path =
            std::env::temp_dir().join(format!("bike-archive-import-{}.zip", uuid::Uuid::new_v4()));
        let file = std::fs::File::create(&archive_path).expect("create archive file");
        let mut writer = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        for (name, bytes) in entries {
            writer
                .start_file(name, options)
                .expect("start archive file");
            writer.write_all(bytes).expect("write archive bytes");
        }

        writer.finish().expect("finish archive");
        archive_path
    }

    async fn test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");

        let schema = Schema::new(db.get_database_backend());
        db.execute(&schema.create_table_from_entity(activities::Entity))
            .await
            .expect("create activities table");
        db.execute(&schema.create_table_from_entity(activity_imports::Entity))
            .await
            .expect("create activity imports table");
        db.execute(&schema.create_table_from_entity(activity_analytics::Entity))
            .await
            .expect("create activity analytics table");
        db.execute(&schema.create_table_from_entity(segments::Entity))
            .await
            .expect("create segments table");
        db.execute(&schema.create_table_from_entity(segment_efforts::Entity))
            .await
            .expect("create segment efforts table");
        db.execute(&schema.create_table_from_entity(user_preferences::Entity))
            .await
            .expect("create user preferences table");
        db.execute(&schema.create_table_from_entity(analytics_user_states::Entity))
            .await
            .expect("create analytics user states table");
        db.execute(&schema.create_table_from_entity(background_tasks::Entity))
            .await
            .expect("create background tasks table");

        db
    }

    fn test_uploads_dir() -> String {
        let uploads_dir = std::env::temp_dir().join(format!(
            "bike-archive-import-uploads-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&uploads_dir).expect("create uploads dir");
        uploads_dir.display().to_string()
    }

    #[test]
    fn parse_archive_url_requires_https() {
        let error = parse_archive_url("http://example.com/export.zip").unwrap_err();

        assert_eq!(error.message, "Archive URL must use https");
    }

    #[test]
    fn resolves_supported_archive_entries() {
        assert_eq!(
            resolve_archive_activity_entry("activities/ride.fit.gz"),
            Some(ArchiveActivityEntry {
                original_filename: "ride.fit".to_string(),
                format: "fit".to_string(),
                gzip_wrapped: true,
            })
        );
        assert_eq!(
            resolve_archive_activity_entry("strava/2018-ride.gpx.gz"),
            Some(ArchiveActivityEntry {
                original_filename: "2018-ride.gpx".to_string(),
                format: "gpx".to_string(),
                gzip_wrapped: true,
            })
        );
        assert_eq!(
            resolve_archive_activity_entry("garmin/ride.TCX"),
            Some(ArchiveActivityEntry {
                original_filename: "ride.TCX".to_string(),
                format: "tcx".to_string(),
                gzip_wrapped: false,
            })
        );
    }

    #[test]
    fn scan_archive_entries_skips_strava_routes_directory() {
        let archive_path = write_test_archive(&[
            ("activities.csv", b"id,name\n1062,Afternoon Ride\n"),
            ("activities/1062.gpx", b"gpx"),
            ("routes.csv", b"name,path\nOh God Why,routes/18.gpx\n"),
            ("routes/18.gpx", b"gpx"),
        ]);

        let scan = scan_archive_entries(&archive_path).expect("scan archive");
        let _ = std::fs::remove_file(&archive_path);

        assert_eq!(scan.total_entries, 4);
        assert_eq!(scan.supported_entry_count, 1);
        assert_eq!(scan.skipped_unsupported_count, 3);
        assert_eq!(scan.supported_entries.len(), 1);
        assert_eq!(scan.supported_entries[0].entry_name, "activities/1062.gpx");
        assert_eq!(
            scan.supported_entries[0].activity_entry.original_filename,
            "1062.gpx"
        );
    }

    #[test]
    fn scan_archive_entries_keeps_generic_archives_path_agnostic() {
        let archive_path = write_test_archive(&[
            ("rides/18.gpx", b"gpx"),
            ("metadata/readme.txt", b"ignored"),
        ]);

        let scan = scan_archive_entries(&archive_path).expect("scan archive");
        let _ = std::fs::remove_file(&archive_path);

        assert_eq!(scan.total_entries, 2);
        assert_eq!(scan.supported_entry_count, 1);
        assert_eq!(scan.skipped_unsupported_count, 1);
        assert_eq!(scan.supported_entries[0].entry_name, "rides/18.gpx");
    }

    #[tokio::test]
    async fn archive_imports_keep_duplicate_detection_enabled() {
        let db = test_db().await;
        let tasks = TaskQueue::new(db.clone());
        let uploads_dir = test_uploads_dir();
        let archive_path = write_test_archive(&[(
            "activities/activity.fit",
            include_bytes!("../tests/fixtures/activity.fit").as_slice(),
        )]);

        let first = import_activity_archive_from_path(
            &db,
            &tasks,
            &uploads_dir,
            "test-user",
            1,
            "archive_import",
            archive_path.display().to_string(),
            &archive_path,
        )
        .await
        .expect("import first archive");
        assert_eq!(first.imported_count, 1);
        assert_eq!(first.duplicate_count, 0);

        let second = import_activity_archive_from_path(
            &db,
            &tasks,
            &uploads_dir,
            "test-user",
            1,
            "archive_import",
            archive_path.display().to_string(),
            &archive_path,
        )
        .await
        .expect("import second archive");
        assert_eq!(second.imported_count, 0);
        assert_eq!(second.duplicate_count, 1);

        assert_eq!(activities::Entity::find().count(&db).await.unwrap(), 1);
        assert_eq!(
            activity_imports::Entity::find().count(&db).await.unwrap(),
            1
        );

        let _ = std::fs::remove_file(&archive_path);
        let _ = std::fs::remove_dir_all(&uploads_dir);
    }

    #[test]
    #[ignore = "large real-archive verification"]
    fn imports_real_export_archives_from_repo_data() {
        let archives = [
            "2026 05 01 strava export_35999641.zip",
            "2026-05-01 Garmin Export.zip",
        ];

        for archive_name in archives {
            let archive_path = repo_data_archive_path(archive_name);
            let scan = scan_archive_entries(&archive_path).expect("scan archive");
            let entries_to_parse = if archive_name == "2026-05-01 Garmin Export.zip" {
                &scan.supported_entries[..scan.supported_entries.len().min(1_500)]
            } else {
                &scan.supported_entries[..]
            };
            let mut parsed_count = 0usize;
            let mut error_samples = Vec::new();

            for indexed_entry in entries_to_parse {
                let bytes = match read_archive_entry_bytes(&archive_path, &indexed_entry.source) {
                    Ok(value) => value,
                    Err(error) => {
                        error_samples.push(format!(
                            "{}: read failed: {}",
                            indexed_entry.entry_name, error.message
                        ));
                        continue;
                    }
                };

                let bytes = match maybe_decode_archive_entry(&indexed_entry.activity_entry, bytes) {
                    Ok(value) => value,
                    Err(error) => {
                        error_samples.push(format!(
                            "{}: gzip decode failed: {}",
                            indexed_entry.entry_name, error
                        ));
                        continue;
                    }
                };

                match crate::activity_summary::summarize_activity_upload(
                    &indexed_entry.activity_entry.original_filename,
                    &indexed_entry.activity_entry.format,
                    &bytes,
                ) {
                    Ok(draft) => {
                        match crate::activity_details::derive_activity_detail_data(
                            &indexed_entry.activity_entry.original_filename,
                            &indexed_entry.activity_entry.format,
                            &bytes,
                        ) {
                            Ok(derived) => {
                                let _ = crate::dedupe::activity_dedupe_key(
                                    &draft,
                                    &derived.route_points,
                                );
                                parsed_count += 1;
                            }
                            Err(error) => error_samples.push(format!(
                                "{}: derive failed: {}",
                                indexed_entry.entry_name, error.message
                            )),
                        }
                    }
                    Err(error) => error_samples.push(format!(
                        "{}: summary failed: {}",
                        indexed_entry.entry_name, error.message
                    )),
                }
            }

            println!(
                "archive={} total={} supported={} parsed_sample={} failed={}",
                archive_name,
                scan.total_entries,
                scan.supported_entry_count,
                parsed_count,
                error_samples.len()
            );

            if archive_name == "2026-05-01 Garmin Export.zip" {
                assert!(scan.supported_entry_count > 1_000);
                assert_eq!(entries_to_parse.len(), 1_500);
            } else {
                assert!(scan.supported_entry_count > 100);
                assert_eq!(entries_to_parse.len(), scan.supported_entries.len());
            }

            assert_eq!(parsed_count, entries_to_parse.len());
            assert!(
                error_samples.is_empty(),
                "archive {} parse failures: {:?}",
                archive_name,
                error_samples.into_iter().take(10).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    #[ignore = "large real-archive verification"]
    fn scans_garmin_export_nested_uploaded_files() {
        let scan = scan_archive_entries(&repo_data_archive_path("2026-05-01 Garmin Export.zip"))
            .expect("scan garmin export");

        println!(
            "garmin total={} supported={} skipped={}",
            scan.total_entries, scan.supported_entry_count, scan.skipped_unsupported_count
        );

        assert!(scan.supported_entry_count > 100);
        assert!(scan
            .supported_entries
            .iter()
            .any(|entry| { matches!(entry.source, ArchiveEntrySource::NestedZip { .. }) }));
    }
}
