use bike_core::{
    strava::{objects::ObjectRef, workflow::steps::*},
    workflow_error::WorkflowError,
};
use std::{future::Future, sync::Arc, time::Duration};
use temporalio_common::error::ApplicationFailure;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};

pub struct StravaActivities {
    steps: ActivitySteps,
}

impl StravaActivities {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            steps: ActivitySteps::new(db),
        }
    }
}

#[activities]
impl StravaActivities {
    #[activity(name = "strava_fetch_activity")]
    pub async fn fetch_activity(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: Operation,
    ) -> Result<Option<ObjectRef>, ActivityError> {
        let id = input.workflow_id.clone();
        execute_step(ctx, &id, self.steps.fetch_activity(input)).await
    }

    #[activity(name = "strava_fetch_streams")]
    pub async fn fetch_streams(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ActivitySource,
    ) -> Result<Option<ObjectRef>, ActivityError> {
        let id = input.workflow_id.clone();
        execute_step(ctx, &id, self.steps.fetch_streams(input)).await
    }

    #[activity(name = "strava_apply_activity")]
    pub async fn apply_activity(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: RecordingSources,
    ) -> Result<ActivityEffect, ActivityError> {
        let id = input.workflow_id.clone();
        execute_step(ctx, &id, self.steps.apply_activity(input)).await
    }

    #[activity(name = "strava_capture_deletion")]
    pub async fn capture_deletion(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: Operation,
    ) -> Result<Option<ObjectRef>, ActivityError> {
        let id = input.workflow_id.clone();
        execute_step(ctx, &id, self.steps.capture_deletion(input)).await
    }

    #[activity(name = "strava_delete_activity")]
    pub async fn delete_activity(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: DeletionSource,
    ) -> Result<ActivityEffect, ActivityError> {
        let id = input.workflow_id.clone();
        execute_step(ctx, &id, self.steps.delete_activity(input)).await
    }

    #[activity(name = "strava_enqueue_derived_work")]
    pub async fn enqueue_derived_work(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: Mutation,
    ) -> Result<DerivedWork, ActivityError> {
        let id = input.workflow_id.clone();
        execute_step(ctx, &id, self.steps.enqueue_derived_work(input)).await
    }

    #[activity(name = "strava_wait_for_publication")]
    pub async fn wait_for_publication(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: DerivedWork,
    ) -> Result<DerivedWork, ActivityError> {
        verify_identity(&ctx, &input.workflow_id)?;
        loop {
            let state = self
                .steps
                .publication_state(&input)
                .await
                .map_err(activity_error)?;
            if let Some(id) = state.failed_task {
                return Err(
                    ApplicationFailure::non_retryable(std::io::Error::other(format!(
                        "Derived task {id} failed or was canceled"
                    )))
                    .into(),
                );
            }
            if state.ready() {
                return Ok(input);
            }
            ctx.record_heartbeat(state).await?;
            tokio::select! {
                () = ctx.cancelled() => return Err(ActivityError::cancelled()),
                () = tokio::time::sleep(Duration::from_secs(1)) => {}
            }
        }
    }

    #[activity(name = "strava_complete_operation")]
    pub async fn complete_operation(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: Completion,
    ) -> Result<OperationResult, ActivityError> {
        let id = input.mutation.workflow_id.clone();
        execute_step(ctx, &id, self.steps.complete_operation(input)).await
    }
}

fn verify_identity(ctx: &ActivityContext, id: &str) -> Result<(), ActivityError> {
    if ctx.info().workflow_id.as_deref() != Some(id) {
        return Err(ApplicationFailure::non_retryable(std::io::Error::other(
            "Activity receipt does not match its Temporal workflow ID",
        ))
        .into());
    }
    if ctx.is_cancelled() {
        return Err(ActivityError::cancelled());
    }
    Ok(())
}

async fn execute_step<T>(
    ctx: ActivityContext,
    id: &str,
    future: impl Future<Output = Result<T, WorkflowError>>,
) -> Result<T, ActivityError> {
    verify_identity(&ctx, id)?;
    tokio::pin!(future);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            result = &mut future => return result.map_err(activity_error),
            () = ctx.cancelled() => return Err(ActivityError::cancelled()),
            _ = heartbeat.tick() => ctx.record_heartbeat(id.to_owned()).await?,
        }
    }
}

pub(super) fn activity_error(error: WorkflowError) -> ActivityError {
    let delay = error.retry_at.map(|at| {
        (at - chrono::Utc::now())
            .to_std()
            .unwrap_or(Duration::from_secs(1))
            .max(Duration::from_secs(1))
    });
    let non_retryable =
        error.status.is_client_error() && error.retry_at.is_none() && error.status.as_u16() != 429;
    ApplicationFailure::builder(error)
        .type_name("StravaStepFailure".to_owned())
        .non_retryable(non_retryable)
        .maybe_next_retry_delay(delay)
        .build()
        .into()
}
