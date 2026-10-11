use super::{graph, stage::json_error, Run};
use crate::{
    entities::{activities, activity_import_artifacts, activity_imports, strava_activity_states},
    jobs::JobQueue,
    strava_activity::ActivityPayload,
    strava_objects::checksum,
    strava_provider_payload::{
        strava_activity_is_bike, StoredStravaProviderPayload, StravaActivityStreams,
        StravaActivitySummary,
    },
    workflow_error::WorkflowError,
};
use sea_orm::{EntityTrait, TransactionTrait};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DeleteCapture {
    activity: Option<activities::Model>,
    segment_ids: Vec<i32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Effect {
    outcome: String,
    hash: Option<String>,
    #[serde(default)]
    activity_id: Option<i32>,
    #[serde(default)]
    import_id: Option<i32>,
}

impl Run<'_> {
    pub(super) async fn activity(&self) -> Result<(), WorkflowError> {
        let id = self
            .row
            .activity_id
            .ok_or_else(|| WorkflowError::bad_request("Missing Strava activity ID"))?;
        let state = strava_activity_states::Entity::find_by_id((self.row.athlete_id, id))
            .one(self.db)
            .await?;
        if state.as_ref().is_some_and(|state| {
            state.connection_generation == self.source.generation
                && state.event_time > self.source.event_time
        }) {
            self.stage("stale", &self.source.event_time.to_string(), async {
                Ok(serde_json::json!({"outcome":"stale"}))
            })
            .await?;
            return Ok(());
        }
        if self.row.action == "delete" {
            self.delete().await
        } else {
            self.upsert().await
        }
    }

    async fn upsert(&self) -> Result<(), WorkflowError> {
        let id = self
            .row
            .activity_id
            .ok_or_else(|| WorkflowError::bad_request("Missing Strava activity ID"))?;
        let mut activity: Option<StravaActivitySummary> = None;
        let mut streams = StravaActivityStreams::default();
        let mut payload: Option<ActivityPayload> = None;
        let mut effect: Option<Effect> = None;
        for node in graph(&self.row.action)? {
            match node {
                "activity" => {
                    activity = Some(
                        self.stage(node, &self.row.id, self.resolve_activity(id))
                            .await?,
                    )
                }
                "streams" => {
                    let activity = activity
                        .as_ref()
                        .ok_or_else(|| WorkflowError::internal("Activity stage is missing"))?;
                    streams = self
                        .stage(
                            node,
                            &stream_revision(activity)?,
                            self.resolve_streams(activity),
                        )
                        .await?;
                }
                "normalize" => {
                    let value = ActivityPayload {
                        activity: activity
                            .clone()
                            .ok_or_else(|| WorkflowError::internal("Activity stage is missing"))?,
                        streams: streams.clone(),
                    };
                    let input = checksum(&serde_json::to_vec(&value).map_err(json_error)?);
                    payload = Some(self.stage(node, &input, async { Ok(value) }).await?);
                }
                "apply" => {
                    let payload = payload
                        .as_ref()
                        .ok_or_else(|| WorkflowError::internal("Normalize stage is missing"))?;
                    let hash = checksum(
                        &crate::strava::build_activity_upload(&payload.activity, &payload.streams)?
                            .bytes,
                    );
                    effect = Some(self.stage(node, &hash, self.apply(payload, &hash)).await?);
                }
                "derived" => {
                    let effect = effect
                        .as_ref()
                        .ok_or_else(|| WorkflowError::internal("Apply stage is missing"))?;
                    self.stage(node, &self.row.id, async { Ok(effect.clone()) })
                        .await?;
                }
                _ => return Err(WorkflowError::internal("Unsupported activity DAG stage")),
            }
        }
        Ok(())
    }

    async fn resolve_activity(&self, id: i64) -> Result<StravaActivitySummary, WorkflowError> {
        let activity = if let Some(activity) = &self.source.activity {
            let mut value = self
                .legacy_payload(id)
                .await?
                .map(|existing| serde_json::to_value(existing.activity))
                .transpose()
                .map_err(json_error)?
                .unwrap_or_else(|| serde_json::json!({}));
            let fields = activity.as_object().ok_or_else(|| {
                WorkflowError::bad_request("Strava activity summary is not an object")
            })?;
            for (key, field) in fields {
                value[key] = field.clone();
            }
            serde_json::from_value(value).map_err(json_error)?
        } else {
            self.archive(
                Some(id),
                &format!(
                    "{}-{}",
                    self.source.event_time,
                    checksum(self.row.id.as_bytes())
                ),
                "activity",
            )
            .scope(self.client.get_activity(&self.token, id))
            .await?
        };
        if activity.id != id {
            return Err(WorkflowError::bad_request(
                "Strava response activity ID does not match the workflow",
            ));
        }
        Ok(activity)
    }

    async fn resolve_streams(
        &self,
        activity: &StravaActivitySummary,
    ) -> Result<StravaActivityStreams, WorkflowError> {
        if !strava_activity_is_bike(activity) {
            return Ok(StravaActivityStreams::default());
        }
        let revision = stream_revision(activity)?;
        let archive = self.archive(Some(activity.id), &revision, "streams");
        if let Some(saved) = archive.store.response(&archive.key).await? {
            return serde_json::from_slice(&archive.store.read(&saved.body).await?)
                .map_err(json_error);
        }
        // Previously retained imports are valid local sources too. Preserve their
        // legacy provenance rather than claiming they contain original HTTP bytes.
        if let Some(existing) = self.legacy_payload(activity.id).await? {
            if stream_revision(&existing.activity)? == revision {
                self.store
                    .transform(
                        &format!(
                            "strava/{}/{}/transforms/legacy-streams/v1/{revision}",
                            self.row.athlete_id, activity.id
                        ),
                        &existing.streams,
                    )
                    .await?;
                return Ok(existing.streams);
            }
        }
        archive
            .scope(self.client.get_activity_streams(&self.token, activity.id))
            .await
    }

    async fn legacy_payload(
        &self,
        id: i64,
    ) -> Result<Option<StoredStravaProviderPayload>, WorkflowError> {
        let Some(activity) =
            activities::Model::strava_activity(self.db, self.connection.user_id, id).await?
        else {
            return Ok(None);
        };
        let Some(import_id) = activity.activity_import_id else {
            return Ok(None);
        };
        let Some(import) =
            activity_imports::Entity::find_owned(self.db, self.connection.user_id, import_id)
                .await?
        else {
            return Ok(None);
        };
        if import.format != "json" {
            return Ok(None);
        }
        let bytes = tokio::fs::read(
            std::path::Path::new(&crate::config::Config::get().uploads_dir)
                .join(import.storage_path),
        )
        .await?;
        serde_json::from_slice(&bytes).map(Some).map_err(json_error)
    }

    async fn apply(&self, payload: &ActivityPayload, hash: &str) -> Result<Effect, WorkflowError> {
        if !strava_activity_is_bike(&payload.activity) {
            if activities::Model::strava_activity(
                self.db,
                self.connection.user_id,
                payload.activity.id,
            )
            .await?
            .is_some()
            {
                self.delete().await?;
            } else {
                self.save_state(Some(hash.into()), false).await?;
            }
            return Ok(Effect {
                outcome: "excluded".into(),
                hash: None,
                activity_id: None,
                import_id: None,
            });
        }
        let mut changed = true;
        if let Some(activity) = activities::Model::strava_activity(
            self.db,
            self.connection.user_id,
            payload.activity.id,
        )
        .await?
        {
            if let Some(import_id) = activity.activity_import_id {
                let artifacts = activity_import_artifacts::Entity::for_import(
                    self.db,
                    self.connection.user_id,
                    import_id,
                )
                .await?;
                changed = !artifacts.iter().any(|artifact| {
                    artifact.artifact_kind == "provider_payload" && artifact.checksum_sha256 == hash
                });
                if !changed {
                    self.resume_import(&activity, import_id).await?;
                }
            }
        }
        if changed {
            let objects = super::import_objects::ImportObjects {
                store: self.store.clone(),
                athlete_id: self.row.athlete_id,
                activity_id: payload.activity.id,
            };
            objects
                .scope(Box::pin(crate::strava_activity::upsert_activity(
                    self.db,
                    &JobQueue::new(self.db.clone()),
                    &crate::config::Config::get().uploads_dir,
                    self.connection.user_id,
                    payload,
                )))
                .await?;
        }
        let activity = activities::Model::strava_activity(
            self.db,
            self.connection.user_id,
            payload.activity.id,
        )
        .await?;
        self.save_state(Some(hash.into()), false).await?;
        Ok(Effect {
            outcome: if changed { "applied" } else { "unchanged" }.into(),
            hash: Some(hash.into()),
            activity_id: activity.as_ref().map(|activity| activity.id),
            import_id: activity.and_then(|activity| activity.activity_import_id),
        })
    }

    async fn resume_import(
        &self,
        activity: &activities::Model,
        import_id: i32,
    ) -> Result<(), WorkflowError> {
        let import =
            activity_imports::Entity::find_owned(self.db, self.connection.user_id, import_id)
                .await?
                .ok_or_else(|| WorkflowError::internal("Strava import record was not found"))?;
        if import.status == crate::activity_import_lifecycle::ACTIVITY_IMPORT_STATUS_PROCESSED {
            return Ok(());
        }
        let profile =
            crate::training_profile::load_training_profile(self.db, self.connection.user_id)
                .await?;
        let objects = super::import_objects::ImportObjects {
            store: self.store.clone(),
            athlete_id: self.row.athlete_id,
            activity_id: self
                .row
                .activity_id
                .ok_or_else(|| WorkflowError::internal("Missing activity correlation ID"))?,
        };
        let result = objects
            .scope(
                crate::activity_import_pipeline::reprocess_activity_from_import(
                    self.db,
                    &crate::config::Config::get().uploads_dir,
                    self.connection.user_id,
                    activity.clone(),
                    import,
                    Some(&profile),
                ),
            )
            .await?;
        crate::activity_import_lifecycle::complete_activity_imports(
            self.db,
            &JobQueue::new(self.db.clone()),
            self.connection.user_id,
            &[import_id],
            result.affected_segment_ids,
            Some(
                activity
                    .started_at
                    .date_naive()
                    .min(result.fitness_dirty_from_day),
            ),
        )
        .await?;
        Ok(())
    }

    async fn delete(&self) -> Result<(), WorkflowError> {
        let mut capture: Option<DeleteCapture> = None;
        for node in graph("delete")? {
            match node {
                "capture_delete" => {
                    capture = Some(
                        self.stage(node, &self.row.id, self.capture_delete())
                            .await?,
                    )
                }
                "delete" => {
                    let capture = capture
                        .as_ref()
                        .ok_or_else(|| WorkflowError::internal("Missing delete checkpoint"))?;
                    self.stage(node, &self.row.id, async {
                        if let Some(activity) = &capture.activity {
                            if activities::Entity::find_by_id(activity.id)
                                .one(self.db)
                                .await?
                                .is_some()
                            {
                                crate::activity_lifecycle::delete_activity_with_derived_state(
                                    self.db,
                                    &crate::config::Config::get().uploads_dir,
                                    self.connection.user_id,
                                    activity.clone(),
                                )
                                .await?;
                            }
                        }
                        self.save_state(None, true).await?;
                        Ok(Effect {
                            outcome: if capture.activity.is_some() {
                                "deleted"
                            } else {
                                "already_absent"
                            }
                            .into(),
                            hash: None,
                            activity_id: capture.activity.as_ref().map(|activity| activity.id),
                            import_id: capture
                                .activity
                                .as_ref()
                                .and_then(|activity| activity.activity_import_id),
                        })
                    })
                    .await?;
                }
                "derived" => {
                    let capture = capture
                        .as_ref()
                        .ok_or_else(|| WorkflowError::internal("Missing delete checkpoint"))?;
                    self.stage("delete_derived", &self.row.id, self.delete_derived(capture))
                        .await?;
                }
                _ => return Err(WorkflowError::internal("Unsupported delete DAG stage")),
            }
        }
        Ok(())
    }

    async fn capture_delete(&self) -> Result<DeleteCapture, WorkflowError> {
        let id = self
            .row
            .activity_id
            .ok_or_else(|| WorkflowError::bad_request("Missing Strava activity ID"))?;
        let activity =
            activities::Model::strava_activity(self.db, self.connection.user_id, id).await?;
        let segment_ids = if let Some(activity) = &activity {
            crate::activity_lifecycle::load_segment_ids_for_activity(self.db, activity.id).await?
        } else {
            Vec::new()
        };
        Ok(DeleteCapture {
            activity,
            segment_ids,
        })
    }

    async fn delete_derived(
        &self,
        capture: &DeleteCapture,
    ) -> Result<serde_json::Value, WorkflowError> {
        if let Some(activity) = &capture.activity {
            let changed_at = self.row.accepted_at;
            crate::analytics::mark_user_fitness_dirty(
                self.db,
                self.connection.user_id,
                activity.started_at.date_naive(),
                changed_at,
            )
            .await?;
            crate::analytics::mark_segment_activity_changes(
                self.db,
                &capture.segment_ids,
                changed_at,
            )
            .await?;
            let tasks = JobQueue::new(self.db.clone());
            tasks
                .rebuild_fitness_freshness(self.connection.user_id)
                .await?;
            tasks
                .rebuild_segment_analytics(capture.segment_ids.clone())
                .await?;
        }
        Ok(serde_json::json!({"outcome":"completed","segment_ids":capture.segment_ids}))
    }

    async fn save_state(&self, hash: Option<String>, deleted: bool) -> Result<(), WorkflowError> {
        let txn = self.db.begin().await?;
        strava_activity_states::Model {
            athlete_id: self.row.athlete_id,
            activity_id: self
                .row
                .activity_id
                .ok_or_else(|| WorkflowError::bad_request("Missing Strava activity ID"))?,
            connection_generation: self.source.generation,
            event_time: self.source.event_time,
            applied_hash: hash,
            deleted,
            applied_at: chrono::Utc::now(),
        }
        .save(&txn)
        .await?;
        txn.commit().await?;
        Ok(())
    }
}

fn stream_revision(activity: &StravaActivitySummary) -> Result<String, WorkflowError> {
    // Names, privacy, and descriptions do not invalidate a recording.
    Ok(checksum(&serde_json::to_vec(&serde_json::json!({"id":activity.id,"start_date":activity.start_date,"elapsed_time":activity.elapsed_time,"moving_time":activity.moving_time,"distance":activity.distance,"upload_id":activity.additional_fields.get("upload_id")})).map_err(json_error)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity() -> StravaActivitySummary {
        let fixture: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../testdata/strava-real-ride.json")).unwrap();
        serde_json::from_value(fixture["activity"].clone()).unwrap()
    }

    #[test]
    fn metadata_update_keeps_the_recording_cache_identity() {
        let original = activity();
        let mut updated = original.clone();
        updated.name = "Edited title".into();
        updated
            .additional_fields
            .insert("private".into(), serde_json::json!(true));
        assert_eq!(
            stream_revision(&original).unwrap(),
            stream_revision(&updated).unwrap()
        );
    }

    #[test]
    fn changed_recording_has_a_new_cache_identity() {
        let original = activity();
        let mut updated = original.clone();
        updated.distance = original.distance.map(|distance| distance + 1.0);
        assert_ne!(
            stream_revision(&original).unwrap(),
            stream_revision(&updated).unwrap()
        );
    }
}
