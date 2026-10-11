use super::{inbox, intents, Run, Source};
use crate::{entities::strava_workflows, workflow_error::WorkflowError};
use sea_orm::EntityTrait;
use tracing::Instrument;

impl Run<'_> {
    pub(super) async fn child_activity(
        &self,
        id: &str,
        action: &str,
        activity_id: i64,
        source: Source,
    ) -> Result<strava_workflows::Model, WorkflowError> {
        inbox::accept(
            self.db,
            &self.connection,
            id,
            action,
            Some(activity_id),
            source.clone(),
        )
        .await?;
        let row = strava_workflows::Entity::find_by_id(id)
            .one(self.db)
            .await?
            .ok_or_else(|| WorkflowError::internal("Strava child workflow was not retained"))?;
        if row.status != "completed" {
            if let Some(raw) = &source.raw {
                self.store.read(raw).await?;
            }
            let child = Run {
                db: self.db,
                row: row.clone(),
                source,
                connection: self.connection.clone(),
                client: self.client.clone(),
                store: self.store.clone(),
                token: self.token.clone(),
            };
            let span = tracing::info_span!(
                "strava.workflow",
                correlation_id = %row.correlation_id(),
                workflow_id = id,
                parent_workflow_id = %self.row.id,
                strava_activity_id = activity_id,
                athlete_id = self.row.athlete_id,
                trace_id = tracing::field::Empty,
                span_id = tracing::field::Empty,
                action
            );
            crate::observability::record_span_trace_context(&span);
            let result = intents::scope(id, child.activity().instrument(span)).await;
            row.finish(
                self.db,
                result.as_ref().err().map(|error| error.message.clone()),
            )
            .await?;
            result?;
        }
        strava_workflows::Entity::find_by_id(id)
            .one(self.db)
            .await?
            .ok_or_else(|| WorkflowError::internal("Strava child workflow disappeared"))
    }
}
