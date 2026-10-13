use super::{activities::StravaActivities, options};
use bike_core::strava::workflow::steps::*;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};
use temporalio_workflow::WorkflowTermination;

#[workflow]
#[derive(Default)]
pub struct StravaActivityUpsert;

#[workflow_methods]
impl StravaActivityUpsert {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: Operation,
    ) -> WorkflowResult<OperationResult> {
        let result = Self::upsert_steps(ctx, input).await;
        if ctx.cancellation_token().is_cancelled() {
            Err(WorkflowTermination::cancelled())
        } else {
            result
        }
    }

    async fn upsert_steps(
        ctx: &mut WorkflowContext<Self>,
        input: Operation,
    ) -> WorkflowResult<OperationResult> {
        let Some(activity) = ctx
            .execute_activity(
                StravaActivities::fetch_activity,
                input.clone(),
                options::step("Fetch Strava activity"),
            )
            .await
            .map_err(options::termination)?
        else {
            return Ok(OperationResult::superseded(input.workflow_id));
        };
        let Some(streams) = ctx
            .execute_activity(
                StravaActivities::fetch_streams,
                ActivitySource {
                    workflow_id: input.workflow_id.clone(),
                    activity: activity.clone(),
                },
                options::step("Fetch Strava streams"),
            )
            .await
            .map_err(options::termination)?
        else {
            return Ok(OperationResult::superseded(input.workflow_id));
        };
        let effect = ctx
            .execute_activity(
                StravaActivities::apply_activity,
                RecordingSources {
                    workflow_id: input.workflow_id.clone(),
                    activity,
                    streams,
                },
                options::step("Apply activity to Bike"),
            )
            .await
            .map_err(options::termination)?;
        let mutation = Mutation {
            workflow_id: input.workflow_id,
            effect,
        };
        let derived = ctx
            .execute_activity(
                StravaActivities::enqueue_derived_work,
                mutation.clone(),
                options::step("Enqueue derived Bike work"),
            )
            .await
            .map_err(options::termination)?;
        let derived = ctx
            .execute_activity(
                StravaActivities::wait_for_publication,
                derived,
                options::publication(),
            )
            .await
            .map_err(options::termination)?;
        ctx.execute_activity(
            StravaActivities::complete_operation,
            Completion { mutation, derived },
            options::step("Record completed Strava upsert"),
        )
        .await
        .map_err(options::termination)
    }
}

#[workflow]
#[derive(Default)]
pub struct StravaActivityDelete;

#[workflow_methods]
impl StravaActivityDelete {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: Operation,
    ) -> WorkflowResult<OperationResult> {
        let result = Self::delete_steps(ctx, input).await;
        if ctx.cancellation_token().is_cancelled() {
            Err(WorkflowTermination::cancelled())
        } else {
            result
        }
    }

    async fn delete_steps(
        ctx: &mut WorkflowContext<Self>,
        input: Operation,
    ) -> WorkflowResult<OperationResult> {
        let Some(capture) = ctx
            .execute_activity(
                StravaActivities::capture_deletion,
                input.clone(),
                options::step("Capture deletion dependencies"),
            )
            .await
            .map_err(options::termination)?
        else {
            return Ok(OperationResult::superseded(input.workflow_id));
        };
        let effect = ctx
            .execute_activity(
                StravaActivities::delete_activity,
                DeletionSource {
                    workflow_id: input.workflow_id.clone(),
                    capture,
                },
                options::step("Delete activity from Bike"),
            )
            .await
            .map_err(options::termination)?;
        let mutation = Mutation {
            workflow_id: input.workflow_id,
            effect,
        };
        let derived = ctx
            .execute_activity(
                StravaActivities::enqueue_derived_work,
                mutation.clone(),
                options::step("Enqueue derived Bike work"),
            )
            .await
            .map_err(options::termination)?;
        let derived = ctx
            .execute_activity(
                StravaActivities::wait_for_publication,
                derived,
                options::publication(),
            )
            .await
            .map_err(options::termination)?;
        ctx.execute_activity(
            StravaActivities::complete_operation,
            Completion { mutation, derived },
            options::step("Record completed Strava deletion"),
        )
        .await
        .map_err(options::termination)
    }
}
