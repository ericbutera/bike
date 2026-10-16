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
    import.require_sql_processing()?;
    let nodes = activity_processing_stages();
    let requested_index = nodes
        .iter()
        .position(|node| node.id() == requested_stage)
        .ok_or_else(|| AppError::bad_request("Select a stage from the activity ingestion stages"))?;
    let artifact = load_best_activity_parsing_artifact(db, import).await?;
    let bytes = tokio::fs::read(Path::new(uploads_dir).join(&artifact.storage_path))
        .await
        .map_err(|_| AppError::bad_request("Retained source is missing or unreadable; restore it before replay"))?;
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
    let previous = find_reusable_attempt(db, import, &source, &nodes[..requested_index], activity.as_ref()).await?;
    let can_reuse = requested_index == 0 || previous.is_some();
    Ok(ActivityImportReplayPlan {
        requested_stage: requested_stage.into(),
        start_stage: if can_reuse { requested_stage } else { "raw_stored" }.into(),
        reused_attempt_id: previous
            .as_ref()
            .filter(|_| requested_index > 0)
            .map(|attempt| attempt.id),
        reason: (!can_reuse)
            .then(|| "Earlier results are missing or stale. This replay must start at Raw stored.".into()),
    })
}

async fn find_reusable_attempt(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    source: &serde_json::Value,
    prerequisites: &[ActivityProcessingStage],
    activity: Option<&activities::Model>,
) -> Result<Option<activity_import_attempts::Model>, AppError> {
    if prerequisites.is_empty() {
        return Ok(None);
    }
    for metadata in activity_import_attempts::Entity::recent_metadata(db, import.user_id, import.id).await? {
        if metadata.source_json != *source {
            continue;
        }
        let Some(attempt) = activity_import_attempts::Entity::find_for_execution(db, metadata.id).await? else {
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
    prerequisites: &[ActivityProcessingStage],
    activity: Option<&activities::Model>,
) -> bool {
    if prerequisites.is_empty() || attempt.source_json != *source {
        return false;
    }
    let Ok(records) = execution::stages(attempt) else {
        return false;
    };
    if !prerequisites.iter().all(|node| {
        records
            .iter()
            .any(|record| record.stage == node.id() && matches!(record.status.as_str(), "completed" | "reused"))
    }) {
        return false;
    }
    if prerequisites.contains(&ActivityProcessingStage::ActivityParsed) {
        let Some(checkpoint) = execution::checkpoint(attempt) else {
            return false;
        };
        if checkpoint.version != 1
            || activity.is_some_and(|activity| checkpoint.activity_updated_at != Some(activity.updated_at))
        {
            return false;
        }
    }
    if prerequisites.contains(&ActivityProcessingStage::ActivitySaved) {
        return activity.is_some_and(|activity| {
            execution::checkpoint(attempt)
                .is_some_and(|checkpoint| checkpoint.activity_updated_at == Some(activity.updated_at))
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
    let plan = plan_activity_import_replay(db, uploads_dir, import, requested_stage).await?;
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
        Some(id) => activity_import_attempts::Entity::find_for_execution(&transaction, id).await?,
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
    import
        .mark_processing(&transaction, &plan.start_stage, None, Utc::now())
        .await?;
    durable::Model::enqueue(
        &transaction,
        "process_activity_import".into(),
        serde_json::json!({"data": {"user_id": import.user_id, "import_id": import.id, "attempt_id": attempt.id}}),
        None,
        1,
    )
    .await?;
    transaction.commit().await?;
    Ok((plan, attempt.id))
}

#[cfg(test)]
mod tests {
    use super::super::test_fixture::RetainedSource;

    #[tokio::test]
    async fn legacy_replay_resumes_fresh_source_and_activity_checkpoint() {
        let fixture = RetainedSource::new().await;
        let db = fixture.database();
        let plan = fixture.plan(&db, "segments_built").await;
        assert_eq!(plan.start_stage, "segments_built");
        assert_eq!(plan.reused_attempt_id, Some(fixture.attempt.id));
        assert!(plan.reason.is_none());
        assert_eq!(db.into_transaction_log().len(), 4);
    }

    #[tokio::test]
    async fn missing_checkpoint_or_changed_activity_requires_replaying_source() {
        for missing_checkpoint in [true, false] {
            let mut fixture = RetainedSource::new().await;
            if missing_checkpoint {
                fixture.attempt.checkpoint_json = None;
            } else {
                fixture.activity.updated_at += chrono::Duration::seconds(1);
            }
            let db = fixture.database();
            let plan = fixture.plan(&db, "segments_built").await;
            assert_eq!(plan.start_stage, "raw_stored");
            assert!(plan.reused_attempt_id.is_none());
            assert!(plan.reason.is_some());
        }
    }

    #[tokio::test]
    async fn full_source_replay_does_not_load_old_attempt_payloads() {
        let fixture = RetainedSource::new().await;
        let db = fixture.database();
        let plan = fixture.plan(&db, "raw_stored").await;
        assert_eq!(plan.start_stage, "raw_stored");
        assert!(plan.reused_attempt_id.is_none());
        assert_eq!(db.into_transaction_log().len(), 2);
    }
}
