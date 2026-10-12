use super::{activity::Effect, rebuild::Rebuild, stage::json_error, Run, Source};
use crate::{
    entities::{strava_connections, strava_workflows},
    strava_objects::checksum,
    workflow_error::WorkflowError,
};
use sea_orm::{ActiveModelTrait, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DiscoveryPage {
    observed_at: chrono::DateTime<chrono::Utc>,
    raw: crate::strava_objects::ObjectRef,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Summary {
    imported: i32,
    unchanged: i32,
    latest: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    rebuild: Rebuild,
}

impl Run<'_> {
    pub(super) async fn collection(&self) -> Result<(), WorkflowError> {
        let mut running: strava_connections::ActiveModel = self.connection.clone().into();
        running.last_sync_status = Set("running".into());
        running.last_sync_started_at = Set(Some(chrono::Utc::now()));
        running.last_sync_message = Set(Some(format!(
            "Strava {} workflow {} is running.",
            self.row.action, self.row.id
        )));
        running.update(self.db).await?;
        let result = self.collection_steps().await;
        if let Err(error) = &result {
            let mut failed: strava_connections::ActiveModel = self.connection.clone().into();
            failed.last_sync_status = Set(if error.status
                == axum::http::StatusCode::TOO_MANY_REQUESTS
            {
                "queued"
            } else {
                "failed"
            }
            .into());
            failed.last_sync_message = Set(Some(error.message.clone()));
            failed.last_sync_failed_count = Set(1);
            failed.update(self.db).await?;
        }
        result
    }

    async fn collection_steps(&self) -> Result<(), WorkflowError> {
        let pages = self
            .stage("discover", &self.row.id, self.pull_pages())
            .await?;
        let summary = self
            .stage("children", &self.row.id, self.apply_pages(&pages))
            .await?;
        self.stage(
            "rebuild",
            &self.row.id,
            summary.rebuild.run(self.db, self.connection.user_id),
        )
        .await?;
        self.complete(&summary).await?;
        Ok(())
    }

    fn after(&self) -> i64 {
        self.source.after.unwrap_or_else(|| {
            (self.row.accepted_at - chrono::Duration::days(super::INITIAL_BACKFILL_DAYS))
                .timestamp()
        })
    }

    async fn pull_pages(&self) -> Result<Vec<DiscoveryPage>, WorkflowError> {
        let mut pages = Vec::new();
        for page in 1..=10_000 {
            let archive = self.archive(None, "", &format!("pages/{page}"));
            let raw = self
                .fetch_source(archive.clone(), |token| async move {
                    self.client
                        .list_activity_page(&token, Some(self.after()), page)
                        .await
                })
                .await?;
            let values: Vec<serde_json::Value> = self.store.read_json(&raw).await?;
            let saved = archive
                .store
                .response(&archive.key)
                .await?
                .ok_or_else(|| WorkflowError::internal("Strava listing was not retained"))?;
            let finished = values.len() < 100;
            pages.push(DiscoveryPage {
                observed_at: saved.observed_at,
                raw,
            });
            if finished {
                return Ok(pages);
            }
        }
        Err(WorkflowError::internal(
            "Strava backfill exceeded its page limit",
        ))
    }

    async fn apply_pages(&self, pages: &[DiscoveryPage]) -> Result<Summary, WorkflowError> {
        let mut summary = Summary::default();
        for page in pages.iter().rev() {
            let values: Vec<serde_json::Value> = self.store.read_json(&page.raw).await?;
            for value in values.iter().rev() {
                let started_at = value
                    .get("start_date")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
                    .ok_or_else(|| {
                        WorkflowError::bad_request(
                            "Strava listing activity has no valid start date",
                        )
                    })?
                    .with_timezone(&chrono::Utc);
                if started_at.timestamp() <= self.after() {
                    continue;
                }
                let id = value
                    .get("id")
                    .and_then(serde_json::Value::as_i64)
                    .ok_or_else(|| {
                        WorkflowError::bad_request("Strava listing activity has no ID")
                    })?;
                let hash = checksum(&serde_json::to_vec(value).map_err(json_error)?);
                let child = format!("{}-{id}-{hash}", self.row.id);
                let source = Source {
                    revoke_remote: false,
                    generation: self.source.generation,
                    event_time: page.observed_at.timestamp(),
                    parent: Some(self.row.id.clone()),
                    activity: None,
                    listing_page: Some(page.raw.clone()),
                    raw: None,
                    after: None,
                };
                let completed = self.child_activity(&child, "upsert", id, source).await?;
                summary.latest = summary.latest.max(Some(started_at));
                let effect = self.child_effect(&completed).await?;
                if effect
                    .as_ref()
                    .is_some_and(|effect| effect.outcome == "applied")
                {
                    summary.imported += 1;
                } else {
                    summary.unchanged += 1;
                }
                if let Some(effect) = effect {
                    summary.rebuild.merge(effect.rebuild);
                }
            }
        }
        Ok(summary)
    }

    async fn child_effect(
        &self,
        row: &strava_workflows::Model,
    ) -> Result<Option<Effect>, WorkflowError> {
        let Some(saved) = row
            .checkpoint
            .as_object()
            .and_then(|stages| stages.iter().find(|(key, _)| key.starts_with("apply:")))
            .map(|(_, value)| value)
        else {
            return Ok(None);
        };
        let object = serde_json::from_value(saved.clone()).map_err(json_error)?;
        self.store.read_json(&object).await.map(Some)
    }

    async fn complete(&self, summary: &Summary) -> Result<Summary, WorkflowError> {
        let mut connection: strava_connections::ActiveModel = self.connection.clone().into();
        connection.last_sync_status = Set("succeeded".into());
        connection.last_sync_finished_at = Set(Some(chrono::Utc::now()));
        connection.last_sync_message = Set(Some(format!(
            "Strava {} completed: {} activities inspected.",
            self.row.action,
            summary.imported + summary.unchanged
        )));
        connection.last_sync_imported_count = Set(summary.imported);
        connection.last_sync_duplicate_count = Set(summary.unchanged);
        connection.last_sync_failed_count = Set(0);
        connection.last_synced_activity_started_at = Set(self
            .connection
            .last_synced_activity_started_at
            .max(summary.latest));
        connection.update(self.db).await?;
        crate::activity_import_lock::release_user_activity_import_lock(
            self.db,
            self.connection.user_id,
            crate::activity_import_lock::ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
        )
        .await?;
        Ok(summary.clone())
    }
}
