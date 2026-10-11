use super::{graph, Run, Source};
use crate::{
    entities::{activities, strava_activity_states},
    workflow_error::WorkflowError,
};
use sea_orm::EntityTrait;

impl Run<'_> {
    pub(super) async fn deauthorize(&self) -> Result<(), WorkflowError> {
        let mut captured: Option<Vec<i64>> = None;
        for stage in graph("deauthorize")? {
            match stage {
                "capture_cleanup" => {
                    captured = Some(
                        self.stage(stage, &self.row.id, self.capture_revocation())
                            .await?,
                    )
                }
                "revoke" => {
                    self.stage(stage, &self.row.id, self.revoke()).await?;
                }
                "cleanup" => {
                    let ids = captured
                        .as_ref()
                        .ok_or_else(|| WorkflowError::internal("Missing revocation capture"))?;
                    self.stage(stage, &self.row.id, self.cleanup_revocation(ids))
                        .await?;
                }
                _ => return Err(WorkflowError::internal("Unsupported revocation DAG stage")),
            }
        }
        Ok(())
    }

    async fn capture_revocation(&self) -> Result<Vec<i64>, WorkflowError> {
        activities::Model::owned_strava_activities(self.db, self.connection.user_id)
            .await?
            .into_iter()
            .map(|activity| {
                activity
                    .source_correlation_id
                    .and_then(|id| id.parse().ok())
                    .ok_or_else(|| {
                        WorkflowError::internal("Strava activity has no vendor correlation ID")
                    })
            })
            .collect()
    }

    async fn revoke(&self) -> Result<serde_json::Value, WorkflowError> {
        if self.source.revoke_remote {
            let connection = crate::strava::ensure_fresh_access_token(
                self.db,
                &self.client,
                self.connection.clone(),
            )
            .await?;
            let token = crate::strava_credentials::open(&connection.access_token)?;
            self.archive(None, "", "deauthorization")
                .scope(self.client.deauthorize(&token))
                .await?;
        }
        Ok(serde_json::json!({"outcome":"revoked","requested_remote":self.source.revoke_remote}))
    }

    async fn cleanup_revocation(
        &self,
        activity_ids: &[i64],
    ) -> Result<serde_json::Value, WorkflowError> {
        for id in activity_ids {
            let state = strava_activity_states::Entity::find_by_id((self.row.athlete_id, *id))
                .one(self.db)
                .await?;
            let event_time = state
                .map_or(self.source.event_time, |state| {
                    state.event_time.max(self.source.event_time)
                })
                .saturating_add(1);
            let source = Source {
                revoke_remote: false,
                generation: self.source.generation,
                event_time,
                parent: Some(self.row.id.clone()),
                activity: None,
                raw: None,
                after: None,
            };
            self.child_activity(
                &format!("{}-delete-{id}", self.row.id),
                "delete",
                *id,
                source,
            )
            .await?;
        }
        crate::strava::forget_local_connection(self.db, self.connection.user_id).await?;
        Ok(serde_json::json!({"outcome":"removed","activities":activity_ids.len()}))
    }
}
