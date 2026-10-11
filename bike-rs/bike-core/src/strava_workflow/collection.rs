use super::{graph, stage::json_error, Run, Source};
use crate::{
    entities::{strava_connections, strava_workflows},
    strava_objects::checksum,
    workflow_error::WorkflowError,
};
use sea_orm::{ActiveModelTrait, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DiscoveryPage {
    values: Vec<serde_json::Value>,
    observed_at: chrono::DateTime<chrono::Utc>,
    raw: crate::strava_objects::ObjectRef,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Summary {
    imported: i32,
    unchanged: i32,
    latest: Option<chrono::DateTime<chrono::Utc>>,
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
        let result = self.collection_graph().await;
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

    async fn collection_graph(&self) -> Result<(), WorkflowError> {
        let mut pages: Option<Vec<DiscoveryPage>> = None;
        let mut summary: Option<Summary> = None;
        for node in graph(&self.row.action)? {
            match node {
                "discover" => pages = Some(self.stage(node, &self.row.id, self.discover()).await?),
                "children" => {
                    let pages = pages
                        .as_ref()
                        .ok_or_else(|| WorkflowError::internal("Missing discovery checkpoint"))?;
                    summary = Some(self.stage(node, &self.row.id, self.children(pages)).await?);
                }
                "complete" => {
                    let summary = summary
                        .as_ref()
                        .ok_or_else(|| WorkflowError::internal("Missing children checkpoint"))?;
                    self.stage(node, &self.row.id, self.complete(summary))
                        .await?;
                }
                _ => return Err(WorkflowError::internal("Unsupported collection DAG stage")),
            }
        }
        Ok(())
    }

    async fn discover(&self) -> Result<Vec<DiscoveryPage>, WorkflowError> {
        let mut pages = Vec::new();
        for page in 1..=10_000 {
            let archive = self.archive(None, "", &format!("pages/{page}"));
            let values = archive
                .clone()
                .scope(
                    self.client
                        .list_activity_page(&self.token, self.source.after, page),
                )
                .await?;
            let saved = archive
                .store
                .response(&archive.key)
                .await?
                .ok_or_else(|| WorkflowError::internal("Strava listing was not retained"))?;
            let finished = values.len() < 100;
            pages.push(DiscoveryPage {
                values,
                observed_at: saved.observed_at,
                raw: saved.body,
            });
            if finished {
                return Ok(pages);
            }
        }
        Err(WorkflowError::internal(
            "Strava backfill exceeded its page limit",
        ))
    }

    async fn children(&self, pages: &[DiscoveryPage]) -> Result<Summary, WorkflowError> {
        let mut summary = Summary::default();
        for page in pages.iter().rev() {
            self.store.read(&page.raw).await?;
            for value in page.values.iter().rev() {
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
                    activity: Some(value.clone()),
                    raw: Some(page.raw.clone()),
                    after: None,
                };
                let completed = self.child_activity(&child, "upsert", id, source).await?;
                let latest = value
                    .get("start_date")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
                    .map(|at| at.with_timezone(&chrono::Utc));
                summary.latest = summary.latest.max(latest);
                let applied = self.child_applied(&completed).await?;
                if applied {
                    summary.imported += 1;
                } else {
                    summary.unchanged += 1;
                }
            }
        }
        Ok(summary)
    }

    async fn child_applied(&self, row: &strava_workflows::Model) -> Result<bool, WorkflowError> {
        let Some(saved) = row
            .checkpoint
            .as_object()
            .and_then(|stages| stages.iter().find(|(key, _)| key.starts_with("apply:")))
            .map(|(_, value)| value)
        else {
            return Ok(false);
        };
        let object = serde_json::from_value(saved.clone()).map_err(json_error)?;
        let output: serde_json::Value =
            serde_json::from_slice(&self.store.read(&object).await?).map_err(json_error)?;
        Ok(output["outcome"] == "applied")
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
