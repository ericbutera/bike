use super::{rebuild::Rebuild, stage::json_error, Action, Run};
use crate::{
    entities::{activities, activity_import_artifacts, activity_imports, strava_activity_states},
    strava::activity::ActivityPayload,
    strava::objects::{checksum, ObjectRef},
    strava::provider_payload::{
        strava_activity_is_bike, StoredStravaProviderPayload, StravaActivityStreams,
        StravaActivitySummary,
    },
    workflow_error::WorkflowError,
};
use sea_orm::{EntityTrait, TransactionTrait};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct DeleteCapture {
    activity: Option<activities::Model>,
    segment_ids: Vec<i32>,
}
impl DeleteCapture {
    fn rebuild(&self) -> Rebuild {
        Rebuild {
            import_ids: Vec::new(),
            segment_ids: self.segment_ids.clone(),
            dirty_from: self
                .activity
                .as_ref()
                .map(|activity| activity.started_at.date_naive()),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub outcome: String,
    hash: Option<String>,
    #[serde(default)]
    pub activity_id: Option<i32>,
    #[serde(default)]
    pub import_id: Option<i32>,
    #[serde(default)]
    pub rebuild: Rebuild,
}

impl Effect {
    pub(super) fn superseded() -> Self {
        Self {
            outcome: "superseded".into(),
            hash: None,
            activity_id: None,
            import_id: None,
            rebuild: Rebuild::default(),
        }
    }
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
        match self.action {
            Action::Delete => self.delete().await,
            Action::Create | Action::Update | Action::Upsert => self.upsert().await,
            _ => Err(WorkflowError::bad_request("Unknown Strava workflow action")),
        }
    }

    async fn upsert(&self) -> Result<(), WorkflowError> {
        let id = self
            .row
            .activity_id
            .ok_or_else(|| WorkflowError::bad_request("Missing Strava activity ID"))?;
        let activity: StravaActivitySummary = self
            .source_stage("activity", &self.row.id, self.resolve_activity(id))
            .await?;
        let streams = self
            .source_stage(
                "streams",
                &stream_revision(&activity)?,
                self.resolve_streams(&activity),
            )
            .await?;
        let payload = ActivityPayload { activity, streams };
        let hash = checksum(
            &crate::strava::build_activity_upload(&payload.activity, &payload.streams)?.bytes,
        );
        let effect = self
            .stage("apply", &hash, self.apply(&payload, &hash))
            .await?;
        // Collection children return their changes to the parent's single rebuild.
        if self.action != Action::Upsert {
            self.stage(
                "rebuild",
                &self.row.id,
                effect.rebuild.run(self.db, self.connection.user_id),
            )
            .await?;
        }
        Ok(())
    }

    pub(super) async fn resolve_activity(&self, id: i64) -> Result<ObjectRef, WorkflowError> {
        let summary = self.source.activity_summary(&self.store, id).await?;
        let object = if let Some(activity) = &summary {
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
            self.store
                .transform(
                    &crate::strava::paths::activity_transform(
                        self.row.athlete_id,
                        id,
                        "listed-activity",
                        &checksum(&serde_json::to_vec(&value).map_err(json_error)?),
                    ),
                    &value,
                )
                .await?
        } else {
            let archive = self.archive(
                &format!(
                    "{}-{}",
                    self.source.event_time,
                    checksum(self.row.id.as_bytes())
                ),
                "activity",
            );
            self.fetch_source(archive, |token| async move {
                self.client.get_activity(&token, id).await
            })
            .await?
        };
        let activity: StravaActivitySummary = self.store.read_json(&object).await?;
        if activity.id != id {
            return Err(WorkflowError::bad_request(
                "Strava response activity ID does not match the workflow",
            ));
        }
        Ok(object)
    }

    pub(super) async fn resolve_streams(
        &self,
        activity: &StravaActivitySummary,
    ) -> Result<ObjectRef, WorkflowError> {
        if !strava_activity_is_bike(activity) {
            return self
                .store
                .transform(
                    &crate::strava::paths::activity_transform(
                        self.row.athlete_id,
                        activity.id,
                        "excluded-streams",
                        "",
                    ),
                    &StravaActivityStreams::default(),
                )
                .await;
        }
        let revision = stream_revision(activity)?;
        let archive = self.archive(&revision, "streams");
        if let Some(saved) = archive.store.response(&archive.key).await? {
            return Ok(saved.body);
        }
        // Previously retained imports are valid local sources too. Preserve their
        // legacy provenance rather than claiming they contain original HTTP bytes.
        if let Some(existing) = self.legacy_payload(activity.id).await? {
            if stream_revision(&existing.activity)? == revision {
                return self
                    .store
                    .transform(
                        &crate::strava::paths::activity_transform(
                            self.row.athlete_id,
                            activity.id,
                            "legacy-streams",
                            &revision,
                        ),
                        &existing.streams,
                    )
                    .await;
            }
        }
        self.fetch_source(archive, |token| async move {
            self.client.get_activity_streams(&token, activity.id).await
        })
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
        let artifact = activity_import_artifacts::Entity::strava_payload(
            self.db,
            self.connection.user_id,
            import_id,
        )
        .await?;
        let object = if let Some(artifact) = artifact {
            ObjectRef {
                path: artifact.storage_path,
                sha256: artifact.checksum_sha256,
                bytes: usize::try_from(artifact.size_bytes)
                    .map_err(|_| WorkflowError::internal("Invalid Strava source size"))?,
            }
        } else if import.format == "json" {
            // Pre-artifact imports remain readable without inventing provenance.
            let bytes = tokio::fs::read(
                std::path::Path::new(&crate::config::Config::get().uploads_dir)
                    .join(import.storage_path),
            )
            .await?;
            return serde_json::from_slice(&bytes).map(Some).map_err(json_error);
        } else {
            return Ok(None);
        };
        let bytes = self.store.read(&object).await?;
        serde_json::from_slice(&bytes).map(Some).map_err(json_error)
    }

    pub(super) async fn apply(
        &self,
        payload: &ActivityPayload,
        hash: &str,
    ) -> Result<Effect, WorkflowError> {
        if !strava_activity_is_bike(&payload.activity) {
            let capture = self
                .stage("capture_delete", &self.row.id, self.capture_delete())
                .await?;
            let mut effect = self.remove_activity(&capture).await?;
            effect.outcome = "excluded".into();
            return Ok(effect);
        }
        let mut changed = true;
        let mut rebuild = Rebuild::default();
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
                    rebuild = self.resume_import(&activity, import_id).await?;
                }
            }
        }
        if changed {
            let objects = super::import_objects::ImportObjects {
                store: self.store.clone(),
                athlete_id: self.row.athlete_id,
                activity_id: payload.activity.id,
            };
            rebuild = objects
                .scope(Box::pin(crate::strava::activity::upsert_activity(
                    self.db,
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
            rebuild,
        })
    }

    async fn resume_import(
        &self,
        activity: &activities::Model,
        import_id: i32,
    ) -> Result<Rebuild, WorkflowError> {
        let import =
            activity_imports::Entity::find_owned(self.db, self.connection.user_id, import_id)
                .await?
                .ok_or_else(|| WorkflowError::internal("Strava import record was not found"))?;
        if import.status == crate::activity_import_lifecycle::ACTIVITY_IMPORT_STATUS_PROCESSED {
            return Ok(Rebuild::default());
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
        Ok(Rebuild::import(
            import_id,
            result.affected_segment_ids,
            activity
                .started_at
                .date_naive()
                .min(result.fitness_dirty_from_day),
        ))
    }

    async fn delete(&self) -> Result<(), WorkflowError> {
        let capture = self
            .stage("capture_delete", &self.row.id, self.capture_delete())
            .await?;
        self.stage("delete", &self.row.id, self.remove_activity(&capture))
            .await?;
        self.stage(
            "rebuild",
            &self.row.id,
            capture.rebuild().run(self.db, self.connection.user_id),
        )
        .await?;
        Ok(())
    }

    pub(super) async fn capture_delete(&self) -> Result<DeleteCapture, WorkflowError> {
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

    pub(super) async fn remove_activity(
        &self,
        capture: &DeleteCapture,
    ) -> Result<Effect, WorkflowError> {
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
            rebuild: capture.rebuild(),
        })
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

pub(super) fn stream_revision(activity: &StravaActivitySummary) -> Result<String, WorkflowError> {
    // Names, privacy, and descriptions do not invalidate a recording.
    Ok(checksum(&serde_json::to_vec(&serde_json::json!({"id":activity.id,"start_date":activity.start_date,"elapsed_time":activity.elapsed_time,"moving_time":activity.moving_time,"distance":activity.distance,"upload_id":activity.additional_fields.get("upload_id")})).map_err(json_error)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity() -> StravaActivitySummary {
        let fixture: serde_json::Value =
            serde_json::from_slice(crate::strava::fixtures::REAL_RIDE).unwrap();
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
