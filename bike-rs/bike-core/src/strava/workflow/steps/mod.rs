//! Domain steps called independently by Temporal activities.
mod session;
mod types;
pub use types::*;

use super::{
    activity::{stream_revision, DeleteCapture},
    Action,
};
use crate::{
    background_jobs::entities::pipeline_runs,
    strava::{
        activity::ActivityPayload,
        objects::{checksum, ObjectRef},
        provider_payload::{StravaActivityStreams, StravaActivitySummary},
    },
    workflow_error::WorkflowError,
};
use sea_orm::DatabaseConnection;
use session::{load_row, Session};

#[derive(Clone)]
pub struct ActivitySteps {
    db: DatabaseConnection,
}

impl ActivitySteps {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn fetch_activity(
        &self,
        input: Operation,
    ) -> Result<Option<ObjectRef>, WorkflowError> {
        let Some(session) =
            Session::load(&self.db, &input.workflow_id, Some(Action::Upsert)).await?
        else {
            return Ok(None);
        };
        let id = session.activity_id()?;
        let _: StravaActivitySummary = session
            .run
            .source_stage(
                "activity",
                &session.run.row.id,
                session.run.resolve_activity(id),
            )
            .await?;
        let reference = session.reference("activity", &session.run.row.id).await?;
        session.release().await?;
        Ok(Some(reference))
    }

    pub async fn fetch_streams(
        &self,
        input: ActivitySource,
    ) -> Result<Option<ObjectRef>, WorkflowError> {
        let Some(session) =
            Session::load(&self.db, &input.workflow_id, Some(Action::Upsert)).await?
        else {
            return Ok(None);
        };
        let activity = session
            .run
            .store
            .read_json::<StravaActivitySummary>(&input.activity)
            .await?;
        ensure_activity_identity(&session, &activity)?;
        let revision = stream_revision(&activity)?;
        let _: StravaActivityStreams = session
            .run
            .source_stage("streams", &revision, session.run.resolve_streams(&activity))
            .await?;
        let reference = session.reference("streams", &revision).await?;
        session.release().await?;
        Ok(Some(reference))
    }

    pub async fn apply_activity(
        &self,
        input: RecordingSources,
    ) -> Result<ActivityEffect, WorkflowError> {
        let Some(session) =
            Session::load(&self.db, &input.workflow_id, Some(Action::Upsert)).await?
        else {
            return Ok(ActivityEffect::superseded());
        };
        let payload = ActivityPayload {
            activity: session.run.store.read_json(&input.activity).await?,
            streams: session.run.store.read_json(&input.streams).await?,
        };
        ensure_activity_identity(&session, &payload.activity)?;
        let hash = checksum(
            &crate::strava::build_activity_upload(&payload.activity, &payload.streams)?.bytes,
        );
        let effect = session
            .with_causal_context(session.run.stage(
                "apply",
                &hash,
                session.run.apply(&payload, &hash),
            ))
            .await?;
        session.release().await?;
        Ok(effect)
    }

    pub async fn capture_deletion(
        &self,
        input: Operation,
    ) -> Result<Option<ObjectRef>, WorkflowError> {
        let Some(session) =
            Session::load(&self.db, &input.workflow_id, Some(Action::Delete)).await?
        else {
            return Ok(None);
        };
        let _: DeleteCapture = session
            .run
            .stage(
                "capture_delete",
                &session.run.row.id,
                session.run.capture_delete(),
            )
            .await?;
        let reference = session
            .reference("capture_delete", &session.run.row.id)
            .await?;
        session.release().await?;
        Ok(Some(reference))
    }

    pub async fn delete_activity(
        &self,
        input: DeletionSource,
    ) -> Result<ActivityEffect, WorkflowError> {
        let Some(session) =
            Session::load(&self.db, &input.workflow_id, Some(Action::Delete)).await?
        else {
            return Ok(ActivityEffect::superseded());
        };
        let expected = session
            .reference("capture_delete", &session.run.row.id)
            .await?;
        if expected.sha256 != input.capture.sha256 || expected.path != input.capture.path {
            return Err(WorkflowError::bad_request(
                "Deletion capture does not belong to its operation",
            ));
        }
        let capture = session
            .run
            .store
            .read_json::<DeleteCapture>(&input.capture)
            .await?;
        let effect = session
            .with_causal_context(session.run.stage(
                "delete",
                &session.run.row.id,
                session.run.remove_activity(&capture),
            ))
            .await?;
        session.release().await?;
        Ok(effect)
    }

    pub async fn enqueue_derived_work(
        &self,
        input: Mutation,
    ) -> Result<DerivedWork, WorkflowError> {
        let Some(session) = Session::load(&self.db, &input.workflow_id, None).await? else {
            return Ok(DerivedWork {
                pipeline_run_id: input.workflow_id.clone(),
                workflow_id: input.workflow_id,
            });
        };
        session
            .with_causal_context(
                session.run.stage(
                    "rebuild",
                    &session.run.row.id,
                    input
                        .effect
                        .rebuild
                        .run(&self.db, session.run.connection.user_id),
                ),
            )
            .await?;
        let work = DerivedWork {
            workflow_id: input.workflow_id,
            pipeline_run_id: session.pipeline.run_id.clone(),
        };
        session.release().await?;
        Ok(work)
    }

    pub async fn publication_state(
        &self,
        input: &DerivedWork,
    ) -> Result<pipeline_runs::PublicationState, WorkflowError> {
        let row = load_row(&self.db, &input.workflow_id).await?;
        if session::origin(&self.db, &row).await?.run_id != input.pipeline_run_id {
            return Err(WorkflowError::bad_request(
                "Publication does not belong to its Strava operation",
            ));
        }
        Ok(pipeline_runs::Model::publication_state(&self.db, &input.pipeline_run_id).await?)
    }

    pub async fn complete_operation(
        &self,
        input: Completion,
    ) -> Result<OperationResult, WorkflowError> {
        if input.mutation.workflow_id != input.derived.workflow_id {
            return Err(WorkflowError::bad_request(
                "Publication belongs to a different Strava operation",
            ));
        }
        let state = self.publication_state(&input.derived).await?;
        if !state.ready() {
            return Err(WorkflowError::conflict(
                "Strava derived outputs are not published",
            ));
        }
        let row = load_row(&self.db, &input.mutation.workflow_id).await?;
        if row.status != "completed" {
            row.finish(&self.db, None).await?;
        }
        Ok(OperationResult {
            workflow_id: row.id,
            outcome: input.mutation.effect.outcome,
            activity_id: input.mutation.effect.activity_id,
            import_id: input.mutation.effect.import_id,
        })
    }
}

fn ensure_activity_identity(
    session: &Session<'_>,
    activity: &StravaActivitySummary,
) -> Result<(), WorkflowError> {
    if activity.id != session.activity_id()? {
        return Err(WorkflowError::bad_request(
            "Strava activity does not match its operation",
        ));
    }
    Ok(())
}
