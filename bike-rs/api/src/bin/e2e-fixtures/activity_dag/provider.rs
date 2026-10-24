//! Only provider responses are replaced; processing and persistence are real.
use super::Result;
use bike_core::workflow_error::WorkflowError;
use integrations::strava::{
    events::StravaEvent,
    native::{PageInput, StravaProvider},
    objects::{ObjectRef, ObjectStore},
    provider_payload::StravaActivitySummary,
};
use serde_json::Value;

pub(super) struct FrozenProvider {
    activity: ObjectRef,
    streams: ObjectRef,
}

impl FrozenProvider {
    pub async fn new(objects: &ObjectStore, payload: &Value) -> Result<Self> {
        Ok(Self {
            activity: objects
                .transform("strava/regression/activity", &payload["activity"])
                .await?,
            streams: objects
                .transform("strava/regression/streams", &payload["streams"])
                .await?,
        })
    }
}

#[async_trait::async_trait]
impl StravaProvider for FrozenProvider {
    async fn activity(&self, _event: &StravaEvent) -> std::result::Result<ObjectRef, WorkflowError> {
        Ok(self.activity.clone())
    }
    async fn streams(&self, _event: &StravaEvent) -> std::result::Result<ObjectRef, WorkflowError> {
        Ok(self.streams.clone())
    }
    async fn page(&self, _input: &PageInput) -> std::result::Result<Vec<StravaActivitySummary>, WorkflowError> {
        Err(WorkflowError::bad_request(
            "The activity regression must not list provider history",
        ))
    }
    async fn revoke(&self, _event: &StravaEvent) -> std::result::Result<(), WorkflowError> {
        Err(WorkflowError::bad_request(
            "The activity regression must not revoke provider access",
        ))
    }
}

/// Fake only provider identity and retained legacy awards; source facts come from the fixed ride.
pub(super) async fn seed(db: &sea_orm::DatabaseConnection, input: &super::Input) -> super::Result<()> {
    use bike_core::entities::{activity_analytics, strava_connections};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    let owner = input.reference["activity"]["user_id"]
        .as_i64()
        .ok_or("Missing reference owner")? as i32;
    if strava_connections::Model::for_user(db, owner).await?.is_none() {
        strava_connections::ActiveModel {
            user_id: Set(owner),
            athlete_id: Set(123456),
            scopes: Set("activity:read_all".into()),
            access_token: Set("fixture".into()),
            refresh_token: Set("fixture".into()),
            expires_at: Set(chrono::Utc::now() + chrono::Duration::days(1)),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    let id = input.reference["activity"]["id"]
        .as_i64()
        .ok_or("Missing reference activity")? as i32;
    if activity_analytics::Entity::find_by_id(id).one(db).await?.is_none() {
        activity_analytics::ActiveModel {
            activity_id: Set(id),
            user_id: Set(owner),
            achievement_highlights_json: Set(serde_json::from_value(
                input.reference["analytics"]["achievement_highlights_json"].clone(),
            )?),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}
