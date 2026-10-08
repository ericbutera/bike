use super::*;
use crate::background_jobs::durable;
use sea_orm::TransactionTrait;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ActivityImportReplayPlan {
    pub requested_stage: String,
    pub start_stage: String,
    pub reused_attempt_id: Option<i32>,
    pub reason: Option<String>,
}

pub async fn plan_activity_import_replay(
    db: &DatabaseConnection,
    uploads_dir: &str,
    import: &activity_imports::Model,
    requested_stage: &str,
) -> Result<ActivityImportReplayPlan, AppError> {
    let nodes = activity_processing_topological_order()?;
    let requested_index = nodes
        .iter()
        .position(|node| node.id() == requested_stage)
        .ok_or_else(|| AppError::bad_request("Select a stage from the activity ingestion graph"))?;
    let artifact = load_best_activity_parsing_artifact(db, import).await?;
    let bytes = tokio::fs::read(Path::new(uploads_dir).join(&artifact.storage_path))
        .await
        .map_err(|_| {
            AppError::bad_request(
                "Retained source is missing or unreadable; restore it before replay",
            )
        })?;
    if artifact
        .checksum_sha256
        .as_ref()
        .is_some_and(|checksum| checksum_sha256_hex(&bytes) != *checksum)
    {
        return Err(AppError::bad_request(
            "Retained source checksum does not match; replay is blocked",
        ));
    }
    let source = super::stage_execution::artifact_metadata(&artifact);
    if artifact.format == "tcx" && crate::activity_parser::is_retired_generated_tcx(&bytes) {
        return Err(AppError::bad_request(
            "Bike-generated activity exports are retired; recover an authentic source",
        ));
    }
    let activity = match import.activity_id {
        Some(id) => activities::Model::find_owned(db, id, import.user_id).await?,
        None => None,
    };
    let previous = find_reusable_attempt(
        db,
        import,
        &source,
        &nodes[..requested_index],
        activity.as_ref(),
    )
    .await?;
    let can_reuse = requested_index == 0 || previous.is_some();
    Ok(ActivityImportReplayPlan {
        requested_stage: requested_stage.into(),
        start_stage: if can_reuse {
            requested_stage
        } else {
            "raw_stored"
        }
        .into(),
        reused_attempt_id: previous
            .as_ref()
            .filter(|_| requested_index > 0)
            .map(|attempt| attempt.id),
        reason: (!can_reuse).then(|| {
            "Earlier results are missing or stale. This replay must start at Raw stored.".into()
        }),
    })
}

async fn find_reusable_attempt(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    source: &serde_json::Value,
    prerequisites: &[ActivityProcessingNode],
    activity: Option<&activities::Model>,
) -> Result<Option<activity_import_attempts::Model>, AppError> {
    if prerequisites.is_empty() {
        return Ok(None);
    }
    for metadata in
        activity_import_attempts::Entity::recent_metadata(db, import.user_id, import.id).await?
    {
        if metadata.source_json != *source {
            continue;
        }
        let Some(attempt) = activity_import_attempts::Entity::find_by_id(metadata.id)
            .one(db)
            .await?
        else {
            continue;
        };
        if reusable_attempt(&attempt, source, prerequisites, activity) {
            return Ok(Some(attempt));
        }
    }
    Ok(None)
}

fn reusable_attempt(
    attempt: &activity_import_attempts::Model,
    source: &serde_json::Value,
    prerequisites: &[ActivityProcessingNode],
    activity: Option<&activities::Model>,
) -> bool {
    if prerequisites.is_empty() || attempt.source_json != *source {
        return false;
    }
    let Ok(records) = execution::stages(attempt) else {
        return false;
    };
    if !prerequisites.iter().all(|node| {
        records.iter().any(|record| {
            record.stage == node.id() && matches!(record.status.as_str(), "completed" | "reused")
        })
    }) {
        return false;
    }
    if prerequisites.contains(&ActivityProcessingNode::ActivityParsed) {
        let Some(checkpoint) = execution::checkpoint(attempt) else {
            return false;
        };
        if checkpoint.version != 1
            || activity
                .is_some_and(|activity| checkpoint.activity_updated_at != Some(activity.updated_at))
        {
            return false;
        }
    }
    if prerequisites.contains(&ActivityProcessingNode::ActivitySaved) {
        return activity.is_some_and(|activity| {
            execution::checkpoint(attempt).is_some_and(|checkpoint| {
                checkpoint.activity_updated_at == Some(activity.updated_at)
            })
        });
    }
    true
}

pub async fn queue_activity_import_replay(
    db: &DatabaseConnection,
    uploads_dir: &str,
    import: &activity_imports::Model,
    requested_stage: &str,
) -> Result<(ActivityImportReplayPlan, i32), AppError> {
    queue_replay(db, uploads_dir, import, requested_stage, None).await
}

pub async fn queue_expected_activity_import_replay(
    db: &DatabaseConnection,
    uploads_dir: &str,
    import: &activity_imports::Model,
    expected: &ActivityImportReplayPlan,
) -> Result<(ActivityImportReplayPlan, i32), AppError> {
    queue_replay(
        db,
        uploads_dir,
        import,
        &expected.requested_stage,
        Some(expected),
    )
    .await
}

async fn queue_replay(
    db: &DatabaseConnection,
    uploads_dir: &str,
    import: &activity_imports::Model,
    requested_stage: &str,
    expected: Option<&ActivityImportReplayPlan>,
) -> Result<(ActivityImportReplayPlan, i32), AppError> {
    let plan = plan_activity_import_replay(db, uploads_dir, import, requested_stage).await?;
    if expected.is_some_and(|expected| expected != &plan) {
        return Err(AppError::conflict(
            "Replay prerequisites changed; review the updated start stage and try again",
        ));
    }
    let source = load_best_activity_parsing_artifact(db, import).await?;
    let transaction = db.begin().await?;
    if activity_import_attempts::Entity::active(&transaction, import.user_id, import.id)
        .await?
        .is_some()
    {
        return Err(AppError::conflict(
            "This import already has a queued or running attempt",
        ));
    }
    let previous = match plan.reused_attempt_id {
        Some(id) => {
            activity_import_attempts::Entity::find_by_id(id)
                .one(&transaction)
                .await?
        }
        None => None,
    };
    let attempt = execution::create_attempt(
        &transaction,
        import,
        super::stage_execution::artifact_metadata(&source),
        &plan.requested_stage,
        &plan.start_stage,
        previous.as_ref(),
    )
    .await?;
    let mut active: activity_imports::ActiveModel = import.clone().into();
    active.status = Set(ACTIVITY_IMPORT_STATUS_PROCESSING.into());
    active.processing_stage = Set(plan.start_stage.clone());
    active.processing_error = Set(None);
    active.last_processing_event_at = Set(Some(Utc::now()));
    active.update(&transaction).await?;
    durable::Model::enqueue(&transaction, "process_activity_import".into(),
        serde_json::json!({"data": {"user_id": import.user_id, "import_id": import.id, "attempt_id": attempt.id}}), None, 1).await?;
    transaction.commit().await?;
    Ok((plan, attempt.id))
}
