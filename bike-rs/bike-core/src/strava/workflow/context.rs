use super::stage::json_error;
use super::Action;
use crate::entities::{strava_connections, strava_workflows};
use crate::strava::{
    client::{RequestArchive, StravaApiClient},
    objects::{ObjectRef, ObjectStore},
    paths::WorkflowPaths,
};
use crate::workflow_error::WorkflowError;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    #[serde(default)]
    pub revoke_remote: bool,
    pub generation: chrono::DateTime<chrono::Utc>,
    pub event_time: i64,
    pub parent: Option<String>,
    pub activity: Option<serde_json::Value>,
    #[serde(default)]
    pub listing_page: Option<ObjectRef>,
    pub raw: Option<ObjectRef>,
    pub after: Option<i64>,
}

impl Source {
    pub(super) async fn verify(&self, store: &ObjectStore) -> Result<(), WorkflowError> {
        for object in [&self.raw, &self.listing_page].into_iter().flatten() {
            store.read(object).await?;
        }
        Ok(())
    }

    pub(super) fn from_value(value: serde_json::Value) -> Result<Self, WorkflowError> {
        serde_json::from_value(value).map_err(json_error)
    }
}

pub(super) struct Run<'a> {
    pub(super) db: &'a DatabaseConnection,
    pub(super) row: strava_workflows::Model,
    pub(super) action: Action,
    pub(super) source: Source,
    pub(super) connection: strava_connections::Model,
    pub(super) client: StravaApiClient,
    pub(super) store: ObjectStore,
}

impl Run<'_> {
    pub(super) fn archive(&self, revision: &str, resource: &str) -> RequestArchive {
        RequestArchive {
            store: self.store.clone(),
            key: WorkflowPaths::new(&self.row).raw(revision, resource),
        }
    }
}
