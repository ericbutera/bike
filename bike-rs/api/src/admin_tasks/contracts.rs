use bike_core::bike_tasks::JobReference;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivityAction {
    Reprocess,
    RebuildFitness,
    RebuildSegments,
    RebuildHeatmap,
    StravaResync,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivityTaskRequest {
    #[schema(value_type = String, format = "uuid")]
    pub command_id: Uuid,
    pub task: ActivityAction,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "task", rename_all = "snake_case", deny_unknown_fields)]
pub enum UserAction {
    RebuildFitness,
    RebuildSegments,
    ReprocessImports {
        #[serde(default)]
        archive_fits_only: bool,
    },
    BackfillXcTraining,
    StravaSync,
    StravaBackfill {
        after: DateTime<Utc>,
        before: DateTime<Utc>,
    },
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UserTaskRequest {
    #[schema(value_type = String, format = "uuid")]
    pub command_id: Uuid,
    #[serde(flatten)]
    pub action: UserAction,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AcceptedTask {
    pub status: String,
    pub job: JobReference,
    pub user_id: i32,
    pub activity_id: Option<i32>,
}

pub struct RequestContext {
    pub actor_id: i32,
    pub request_id: Option<String>,
}
