use super::{Action, Execution, Run, Source};
use crate::{
    entities::{activities, strava_activity_states},
    workflow_error::WorkflowError,
};
use sea_orm::EntityTrait;

impl Run<'_> {
    pub(super) async fn deauthorize(&self) -> Result<Execution, WorkflowError> {
        let ids = self
            .stage("capture_cleanup", &self.row.id, self.capture_revocation())
            .await?;
        self.stage("revoke", &self.row.id, self.revoke()).await?;
        let complete = self
            .stage_when_ready("cleanup", &self.row.id, self.cleanup_revocation(&ids))
            .await?;
        Ok(if complete.is_some() {
            Execution::Completed
        } else {
            Execution::WaitingForChildren
        })
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
            let archive = self.archive("", "deauthorization");
            if archive.store.response(&archive.key).await?.is_none() {
                let token = self.access_token().await?;
                archive.scope(self.client.deauthorize(&token)).await?;
            }
        }
        Ok(serde_json::json!({"outcome":"revoked","requested_remote":self.source.revoke_remote}))
    }

    async fn cleanup_revocation(
        &self,
        activity_ids: &[i64],
    ) -> Result<Option<serde_json::Value>, WorkflowError> {
        let mut waiting = false;
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
                listing_page: None,
                raw: None,
                after: None,
            };
            let child = self
                .child_activity(
                    &format!("{}-delete-{id}", self.row.id),
                    Action::Delete,
                    *id,
                    source,
                )
                .await?;
            waiting |= !self.child_completed(&child).await?;
        }
        if waiting {
            return Ok(None);
        }
        crate::strava::forget_local_connection(self.db, self.connection.user_id).await?;
        Ok(Some(
            serde_json::json!({"outcome":"removed","activities":activity_ids.len()}),
        ))
    }
}
