use super::super::options;
use crate::scheduling::SchedulingActivities;
use bike_core::bike_tasks::{ActivityImportResult, FitnessChange, Priority, SegmentChange};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

pub(crate) async fn refresh_import_outputs<W>(
    boundary: &options::CommitBoundary<'_, W>,
    imported: &ActivityImportResult,
) -> WorkflowResult<()> {
    let request = &imported.fitness;
    boundary
        .run(
            SchedulingActivities::schedule_fitness_freshness,
            request.clone(),
            options::activity(request.priority, format!("user-{}", request.user_id)),
        )
        .await?;
    if !imported.segments.is_empty() {
        let priority = imported
            .segments
            .iter()
            .map(|change| change.priority)
            .min()
            .unwrap_or(Priority::Background);
        boundary
            .run(
                SchedulingActivities::schedule_segment_analytics,
                imported.segments.clone(),
                options::activity(priority, "segment-handoffs".into()),
            )
            .await?;
    }
    if let Some(request) = &imported.heatmap {
        boundary
            .run(
                SchedulingActivities::schedule_heatmap,
                request.clone(),
                options::activity(request.priority, format!("user-{}", request.user_id)),
            )
            .await?;
    }
    Ok(())
}

pub(crate) async fn fitness<W>(ctx: &WorkflowContext<W>, request: &FitnessChange) -> WorkflowResult<()> {
    let options = options::activity(request.priority, format!("user-{}", request.user_id));
    options::run(
        ctx,
        SchedulingActivities::schedule_fitness_freshness,
        request.clone(),
        options,
    )
    .await
}

pub(crate) async fn segments<W>(ctx: &WorkflowContext<W>, requests: &[SegmentChange]) -> WorkflowResult<()> {
    if requests.is_empty() {
        return Ok(());
    }
    let priority = requests
        .iter()
        .map(|request| request.priority)
        .min()
        .unwrap_or(Priority::Background);
    let options = options::activity(priority, "segment-handoffs".into());
    options::run(
        ctx,
        SchedulingActivities::schedule_segment_analytics,
        requests.to_vec(),
        options,
    )
    .await
}
