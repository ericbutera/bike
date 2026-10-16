use super::steps::{derived, segments};
use bike_core::bike_tasks::{ActivitySegmentsInput, ActivitySegmentsResult};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct RegenerateActivitySegments;

#[workflow_methods]
impl RegenerateActivitySegments {
    #[run(name = "regenerate_activity_segments")]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: ActivitySegmentsInput,
    ) -> WorkflowResult<ActivitySegmentsResult> {
        let matched = segments::match_activity(ctx, input).await?;
        derived::segments(ctx, &matched.segments).await?;
        Ok(matched)
    }
}
