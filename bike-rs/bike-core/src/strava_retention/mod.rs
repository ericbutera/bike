//! Thirty-day temporary storage, with durable local activity rebuild sources.
mod files;
mod metrics;
mod protection;
#[cfg(test)]
mod tests;

use crate::{
    entities::{activity_import_artifacts, activity_imports, integration_events, strava_workflows},
    strava_objects::{ObjectRef, ObjectStore},
    workflow_error::WorkflowError,
};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, TransactionTrait};
use std::collections::BTreeSet;

pub const TTL_DAYS: i64 = 30;

pub fn init_metrics() {
    metrics::init();
}

#[derive(Debug, Default)]
pub struct RetentionResult {
    pub files: u64,
    pub bytes: u64,
    pub workflows: u64,
    pub events: u64,
}

pub async fn run(
    db: &DatabaseConnection,
    uploads_dir: &str,
    now: DateTime<Utc>,
) -> Result<RetentionResult, WorkflowError> {
    let lease = if db.get_database_backend() == DatabaseBackend::Postgres {
        let lease = db.begin().await?;
        lease
            .execute_unprepared("SELECT pg_advisory_xact_lock(7393, 0)")
            .await?;
        Some(lease)
    } else {
        None
    };
    let result = retain(db, uploads_dir, now).await?;
    if let Some(lease) = lease {
        lease.commit().await?;
    }
    Ok(result)
}

async fn retain(
    db: &DatabaseConnection,
    uploads_dir: &str,
    now: DateTime<Utc>,
) -> Result<RetentionResult, WorkflowError> {
    let cutoff = now - Duration::days(TTL_DAYS);
    let store = ObjectStore::new(uploads_dir);
    let sources = verify_sources(db, &store).await?;
    let (protected, expired) = protect_work(db, &store, cutoff).await?;
    let inventory = files::inventory(uploads_dir, &sources).await?;
    let mut result = files::prune(&store, &inventory, &protected, cutoff).await?;
    // Remove only history whose input/output objects have expired. Active trees
    // and all current import/artifact references remain untouched.
    let txn = db.begin().await?;
    result.events = integration_events::Entity::prune_strava_workflows(&txn, &expired).await?;
    result.workflows = strava_workflows::Entity::prune_completed(&txn, &expired, cutoff).await?;
    txn.commit().await?;
    metrics::publish(&inventory, &sources, &protected, cutoff);
    tracing::info!(
        files = result.files,
        bytes = result.bytes,
        workflows = result.workflows,
        events = result.events,
        ttl_days = TTL_DAYS,
        "Pruned expired Strava storage; retained activity sources and active work"
    );
    Ok(result)
}

async fn protect_work(
    db: &DatabaseConnection,
    store: &ObjectStore,
    cutoff: DateTime<Utc>,
) -> Result<(protection::Protection, Vec<String>), WorkflowError> {
    let mut protected = protection::Protection::default();
    for path in activity_import_artifacts::Entity::storage_paths(db)
        .await?
        .into_iter()
        .chain(activity_imports::Entity::storage_paths(db).await?)
    {
        store.path(&path)?;
        protected.paths.insert(path);
    }
    let history = strava_workflows::Entity::retention_history(db).await?;
    let queued =
        crate::background_jobs::background_tasks::Model::active_strava_workflow_ids(db).await?;
    let expired = protected
        .workflows(store, &history, &queued, cutoff)
        .await?;
    Ok((protected, expired))
}

async fn verify_sources(
    db: &DatabaseConnection,
    store: &ObjectStore,
) -> Result<BTreeSet<String>, WorkflowError> {
    let mut sources = BTreeSet::new();
    for artifact in activity_import_artifacts::Entity::strava_sources(db).await? {
        let object = ObjectRef {
            path: artifact.storage_path,
            sha256: artifact.checksum_sha256,
            bytes: usize::try_from(artifact.size_bytes)
                .map_err(|_| WorkflowError::internal("Invalid Strava rebuild source size"))?,
        };
        store.read(&object).await?;
        sources.insert(object.path);
    }
    Ok(sources)
}
