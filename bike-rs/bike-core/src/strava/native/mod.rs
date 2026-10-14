//! Provider and domain operations, independent of any job or workflow engine.
mod dependencies;
mod persistence;
use dependencies::{object_key, ProviderClient};
pub use dependencies::{ActivityStorage, StravaProvider};
mod scan;
#[cfg(test)]
pub(crate) mod strava_native_unit;
mod types;
pub use types::*;

use super::{
    client::{RequestArchive, StravaApiClient},
    events::StravaEvent,
    objects::{ObjectRef, ObjectStore},
    provider_payload::{strava_activity_is_bike, StravaActivityStreams, StravaActivitySummary},
};
use crate::{
    config::Config,
    entities::{activities, strava_connections},
    workflow_error::WorkflowError,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct StravaOperations {
    pub(crate) db: DatabaseConnection,
    store: Arc<dyn ActivityStorage>,
    provider: Arc<dyn StravaProvider>,
}

impl StravaOperations {
    pub fn new(db: DatabaseConnection) -> Self {
        let store = ObjectStore::new(&Config::get().uploads_dir);
        let provider = ProviderClient {
            db: db.clone(),
            store: store.clone(),
        };
        Self::with_dependencies(db, Arc::new(provider), Arc::new(store))
    }

    pub fn with_dependencies(
        db: DatabaseConnection,
        provider: Arc<dyn StravaProvider>,
        store: Arc<dyn ActivityStorage>,
    ) -> Self {
        Self {
            db,
            provider,
            store,
        }
    }

    pub(crate) async fn connection(
        &self,
        event: &StravaEvent,
    ) -> Result<strava_connections::Model, WorkflowError> {
        let connection = strava_connections::Model::for_event(&self.db, event).await?;
        self.validate_customer(event).await?;
        Ok(connection)
    }

    async fn validate_customer(&self, event: &StravaEvent) -> Result<(), WorkflowError> {
        let customer =
            crate::auth::entities::users::Model::find_by_id(&self.db, event.bike_user_id).await?;
        event.validate_customer(&customer)
    }

    pub async fn fetch_activity(
        &self,
        event: StravaEvent,
    ) -> Result<ActivitySource, WorkflowError> {
        let id = event.activity_id()?;
        self.connection(&event).await?;
        let source = self.provider.activity(&event).await?;
        let activity: StravaActivitySummary = self.store.read_json(&source).await?;
        if activity.id != id {
            return Err(WorkflowError::bad_request(
                "Strava returned a different activity",
            ));
        }
        Ok(ActivitySource {
            event,
            activity: source,
        })
    }

    pub async fn fetch_streams(
        &self,
        input: ActivitySource,
    ) -> Result<ActivitySources, WorkflowError> {
        let activity: StravaActivitySummary = self.store.read_json(&input.activity).await?;
        self.connection(&input.event).await?;
        let manual = activity
            .additional_fields
            .get("manual")
            .and_then(serde_json::Value::as_bool)
            == Some(true);
        let streams = if strava_activity_is_bike(&activity) && !manual {
            self.provider.streams(&input.event).await?
        } else {
            self.store
                .transform(
                    &self.key(&input.event, "streams"),
                    &StravaActivityStreams::default(),
                )
                .await?
        };
        Ok(ActivitySources {
            event: input.event,
            activity: input.activity,
            streams,
        })
    }

    pub async fn normalize_activity(
        &self,
        input: ActivitySources,
    ) -> Result<NormalizedActivity, WorkflowError> {
        let activity = self
            .store
            .read_json::<StravaActivitySummary>(&input.activity)
            .await?;
        if activity.id != input.event.activity_id()? {
            return Err(WorkflowError::bad_request(
                "Strava source does not match its event",
            ));
        }
        let streams = self
            .store
            .read_json::<StravaActivityStreams>(&input.streams)
            .await?;
        let upload = super::build_activity_upload(&activity, &streams)?;
        let parsed = crate::activity_parser::parse_activity_artifact(
            crate::activity_parser::ActivityParserArtifact {
                original_filename: &upload.original_filename,
                format: "json",
                artifact_kind: "provider_payload",
                source_quality: "strava_streams",
                bytes: &upload.bytes,
            },
        )?;
        let source = self
            .store
            .write(&self.key(&input.event, "normalized-source"), &upload.bytes)
            .await?;
        let parsed = self
            .store
            .transform(&self.key(&input.event, "normalized-activity"), &parsed)
            .await?;
        Ok(NormalizedActivity {
            event: input.event,
            source,
            parsed: Some(parsed),
        })
    }

    pub async fn list_page(&self, input: PageInput) -> Result<ActivityPage, WorkflowError> {
        let event = &input.collection.event;
        self.connection(event).await?;
        let values = self.provider.page(&input).await?;
        let next_page = (!values.is_empty()).then_some(input.page + 1);
        let values = values
            .into_iter()
            .filter(|activity| {
                input
                    .collection
                    .window
                    .before
                    .is_none_or(|before| activity.start_date.timestamp() < before)
            })
            .collect::<Vec<_>>();
        Ok(ActivityPage {
            activity_ids: values.iter().map(|activity| activity.id).collect(),
            next_page,
            latest_started_at: values.iter().map(|activity| activity.start_date).max(),
        })
    }

    fn key(&self, event: &StravaEvent, resource: &str) -> String {
        object_key(event, resource)
    }

    pub async fn collection_window(
        &self,
        event: StravaEvent,
    ) -> Result<CollectionInput, WorkflowError> {
        let connection = self.connection(&event).await?;
        let window = if let Some(window) = &event.window {
            window.clone()
        } else if event.operation == super::events::StravaOperation::Backfill {
            event.window.clone().ok_or_else(|| {
                WorkflowError::bad_request("Backfill requires explicit start and end dates")
            })?
        } else {
            ActivityWindow {
                after: connection
                    .last_synced_activity_started_at
                    .map_or(event.event_time - 15 * 86400, |at| at.timestamp() - 1),
                before: Some(event.event_time + 1),
            }
        };
        if window.before.is_some_and(|before| before <= window.after) {
            return Err(WorkflowError::bad_request(
                "Strava date range must end after it starts",
            ));
        }
        Ok(CollectionInput { event, window })
    }

    pub async fn accept_child(&self, event: StravaEvent) -> Result<StravaEvent, WorkflowError> {
        super::payload::validate_temporal(&event)?;
        self.connection(&event).await?;
        let stored = crate::entities::strava_events::Model::accept(&self.db, &event).await?;
        let event = serde_json::from_value(stored.event.clone())
            .map_err(|error| WorkflowError::internal(error.to_string()))?;
        stored.submitted(&self.db).await?;
        Ok(event)
    }

    pub async fn complete_sync(
        &self,
        mut result: CollectionResult,
    ) -> Result<CollectionResult, WorkflowError> {
        self.connection(&result.event).await?;
        use sea_orm::TransactionTrait;
        let txn = self.db.begin().await?;
        (result.imported, result.unchanged) =
            crate::entities::strava_events::Model::collection_counts(&txn, &result.event).await?;
        strava_connections::Model::complete_native_sync(&txn, &result).await?;
        txn.commit().await?;
        Ok(result)
    }

    pub async fn complete_backfill(
        &self,
        result: CollectionResult,
    ) -> Result<CollectionResult, WorkflowError> {
        self.complete_sync(result).await
    }

    pub async fn begin_disconnect(&self, event: StravaEvent) -> Result<Vec<i64>, WorkflowError> {
        self.connection(&event).await?;
        use sea_orm::TransactionTrait;
        let txn = self.db.begin().await?;
        strava_connections::Model::begin_disconnect(&txn, &event).await?;
        let ids = activities::Model::native_strava_ids(&txn, event.bike_user_id).await?;
        txn.commit().await?;
        Ok(ids)
    }

    pub async fn revoke_connection(
        &self,
        event: StravaEvent,
    ) -> Result<StravaEvent, WorkflowError> {
        if event.revoke_remote {
            self.connection(&event).await?;
            let result = self.provider.revoke(&event).await;
            if let Err(error) = result {
                if error.status.as_u16() != 401 {
                    return Err(error);
                }
            }
        }
        Ok(event)
    }

    pub async fn remove_connection(
        &self,
        event: StravaEvent,
    ) -> Result<StravaEvent, WorkflowError> {
        strava_connections::Model::remove_native(&self.db, &event).await?;
        Ok(event)
    }
}
