use super::*;

pub(super) async fn begin_processing_attempt(
    run: &ActivityProcessingRun<'_>,
) -> Result<activity_import_attempts::Model, AppError> {
    let attempt = match activity_import_attempts::Entity::active(run.db, run.user_id, run.import.id)
        .await?
    {
        Some(attempt) => attempt,
        None => {
            let source = load_best_activity_parsing_artifact(run.db, &run.import).await
                .map(|artifact| artifact_metadata(&artifact))
                .unwrap_or_else(|_| serde_json::json!({"filename": run.import.original_filename, "format": run.import.format}));
            execution::create_attempt(
                run.db,
                &run.import,
                source,
                "raw_stored",
                "raw_stored",
                None,
            )
            .await?
        }
    };
    if !activity_import_attempts::Entity::claim(run.db, attempt.id).await? {
        return Err(AppError::conflict(
            "This import already has a running attempt",
        ));
    }
    activity_import_attempts::Entity::find_by_id(attempt.id)
        .one(run.db)
        .await?
        .ok_or_else(|| AppError::internal("Ingestion attempt disappeared"))
}

pub(super) fn artifact_metadata(artifact: &ActivityProcessingArtifact) -> serde_json::Value {
    serde_json::json!({
        "filename": artifact.original_filename, "format": artifact.format,
        "quality": artifact.source_quality, "checksum": artifact.checksum_sha256,
        "storage_path": artifact.storage_path, "size_bytes": artifact.size_bytes,
    })
}

pub(super) async fn start_processing_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    stage: &str,
) -> Result<(), AppError> {
    state.attempt = execution::record_stage(
        run.db,
        state.attempt.clone(),
        StageRecord {
            stage: stage.into(),
            status: "running".into(),
            started_at: Some(Utc::now()),
            completed_at: None,
            summary: Vec::new(),
            error: None,
            reused_attempt_id: None,
        },
        None,
        state.activity_model.as_ref().map(|activity| activity.id),
    )
    .await?;
    let mut active: activity_imports::ActiveModel = state.import_model.clone().into();
    active.status = Set(ACTIVITY_IMPORT_STATUS_PROCESSING.into());
    active.processing_stage = Set(stage.into());
    active.processing_error = Set(None);
    active.last_processing_event_at = Set(Some(Utc::now()));
    state.import_model = active.update(run.db).await?;
    Ok(())
}

pub(super) async fn complete_processing_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    node: ActivityProcessingNode,
) -> Result<(), AppError> {
    if let Some(activity) = &state.activity_model {
        state.activity_model =
            activities::Model::find_owned(run.db, activity.id, run.user_id).await?;
    }
    let started_at = execution::stages(&state.attempt)?
        .into_iter()
        .find(|stage| stage.stage == node.id())
        .and_then(|stage| stage.started_at);
    let checkpoint = state
        .parsed_activity
        .as_ref()
        .map(|parsed| ProcessingCheckpoint {
            version: 1,
            parsed: parsed.clone(),
            affected_segment_ids: state.affected_segment_ids.clone(),
            activity_updated_at: state
                .activity_model
                .as_ref()
                .map(|activity| activity.updated_at),
        });
    state.attempt = execution::record_stage(
        run.db,
        state.attempt.clone(),
        StageRecord {
            stage: node.id().into(),
            status: "completed".into(),
            started_at,
            completed_at: Some(Utc::now()),
            summary: stage_summary(state, node),
            error: None,
            reused_attempt_id: None,
        },
        checkpoint.as_ref(),
        state.activity_model.as_ref().map(|activity| activity.id),
    )
    .await?;
    Ok(())
}

fn stage_summary(state: &ActivityProcessingState, node: ActivityProcessingNode) -> Vec<String> {
    match node {
        ActivityProcessingNode::RawStored => vec![
            format!(
                "{} · {} bytes",
                state.parsing_artifact.format.to_uppercase(),
                state.bytes.len()
            ),
            format!("Source quality: {}", state.parsing_artifact.source_quality),
            if state.parsing_artifact.checksum_sha256.is_some() {
                "Retained source checksum verified".into()
            } else {
                "Retained source readable; no original checksum recorded".into()
            },
        ],
        ActivityProcessingNode::ActivityParsed => {
            state
                .parsed_activity
                .as_ref()
                .map_or_else(Vec::new, |parsed| {
                    vec![
                        format!("Parsed {} records", parsed.derived_data.route_points.len()),
                        format!(
                            "{} chart samples · {} laps",
                            parsed.derived_data.chart_points.len(),
                            parsed.derived_data.laps.len()
                        ),
                        format!("Sport: {}", parsed.draft.sport),
                    ]
                })
        }
        ActivityProcessingNode::ActivitySaved => state
            .activity_model
            .as_ref()
            .map_or_else(Vec::new, |activity| {
                vec![format!("Saved activity #{}", activity.id)]
            }),
        ActivityProcessingNode::SegmentsBuilt => vec![format!(
            "{} affected segments",
            state.affected_segment_ids.len()
        )],
        ActivityProcessingNode::SegmentAnalyticsBuilt => vec![format!(
            "Rebuilt analytics for {} segments",
            state.affected_segment_ids.len()
        )],
        ActivityProcessingNode::ActivityAnalyticsBuilt => {
            vec!["Rebuilt analytics for 1 activity".into()]
        }
        ActivityProcessingNode::TrainingAnalysisBuilt => vec![if state
            .activity_model
            .as_ref()
            .is_some_and(activities::Model::is_bike_activity)
        {
            "Rebuilt training analysis for 1 activity".into()
        } else {
            "Training analysis skipped: activity is not cycling".into()
        }],
    }
}
