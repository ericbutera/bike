//! Transport adapters enter one source-save and derived-output sequence.
pub(super) mod derived;
pub(super) mod imports;
use super::options;
use crate::activities::ImportActivities;
use bike_core::bike_tasks::{ActivityProcessingResult, ActivityProcessingSource, Priority};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

pub(super) async fn process<W>(
    ctx: &WorkflowContext<W>,
    source: ActivityProcessingSource,
) -> WorkflowResult<ActivityProcessingResult> {
    let result = match source {
        ActivityProcessingSource::Import(command) => {
            ActivityProcessingResult::Import(imports::activity_import(ctx, *command).await?)
        }
        ActivityProcessingSource::Strava(source) => {
            let options = options::activity(Priority::NewActivity, format!("user-{}", source.event.bike_user_id));
            options::run(
                ctx,
                ImportActivities::process_activity,
                ActivityProcessingSource::Strava(source),
                options,
            )
            .await?
        }
    };
    if let Some(activity) = result.activity().map_err(temporalio_sdk::ApplicationFailure::new)? {
        derived::rebuild(ctx, &activity).await?;
    }
    Ok(result)
}
