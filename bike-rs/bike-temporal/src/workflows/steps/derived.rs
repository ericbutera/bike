use super::super::options;
use crate::scheduling::SchedulingActivities;
use bike_core::bike_tasks::{
    ActivityImportResult, FitnessChange, HeatmapChange, Priority, SegmentChange,
};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

pub(crate) async fn refresh_import_outputs<W>(
    ctx: &WorkflowContext<W>,
    imported: &ActivityImportResult,
) -> WorkflowResult<()> {
    fitness(ctx, &imported.fitness).await?;
    segments(ctx, &imported.segments).await?;
    if let Some(request) = &imported.heatmap {
        heatmap(ctx, request).await?;
    }
    Ok(())
}

pub(crate) async fn fitness<W>(
    ctx: &WorkflowContext<W>,
    request: &FitnessChange,
) -> WorkflowResult<()> {
    let options = options::activity(request.priority, format!("user-{}", request.user_id));
    options::run(
        ctx,
        SchedulingActivities::schedule_fitness_freshness,
        request.clone(),
        options,
    )
    .await
}

pub(crate) async fn segments<W>(
    ctx: &WorkflowContext<W>,
    requests: &[SegmentChange],
) -> WorkflowResult<()> {
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

pub(crate) async fn heatmap<W>(
    ctx: &WorkflowContext<W>,
    request: &HeatmapChange,
) -> WorkflowResult<()> {
    let options = options::activity(request.priority, format!("user-{}", request.user_id));
    options::run(
        ctx,
        SchedulingActivities::schedule_heatmap,
        request.clone(),
        options,
    )
    .await
}
