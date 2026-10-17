use super::super::{children, options, PrepareHeatmap, RebuildFitnessFreshness, RebuildSegmentAnalytics};
use crate::activities::DerivedActivities;
use bike_core::bike_tasks::{ActivityImportResult, ActivityTask, FitnessChange, SegmentChange};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

pub(crate) async fn segments<W>(ctx: &WorkflowContext<W>, imported: &ActivityImportResult) -> WorkflowResult<()> {
    activity_segments(ctx, imported.activity()).await
}

pub(crate) async fn activity_segments<W>(ctx: &WorkflowContext<W>, task: ActivityTask) -> WorkflowResult<()> {
    let priority = task.priority;
    let options = options::activity(task.priority, format!("user-{}", task.user_id));
    let segment_ids = options::run(ctx, DerivedActivities::rebuild_activity_segments, task, options).await?;
    for segment_id in segment_ids {
        let task = SegmentChange { segment_id, priority };
        children::run(
            ctx,
            RebuildSegmentAnalytics::run,
            task,
            priority,
            format!("segment-{segment_id}"),
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn achievements<W>(ctx: &WorkflowContext<W>, imported: &ActivityImportResult) -> WorkflowResult<()> {
    let task = imported.activity();
    let options = options::activity(task.priority, format!("user-{}", task.user_id));
    options::run(ctx, DerivedActivities::rebuild_activity_achievements, task, options).await
}

pub(crate) async fn heatmap<W>(ctx: &WorkflowContext<W>, imported: &ActivityImportResult) -> WorkflowResult<()> {
    children::run(
        ctx,
        PrepareHeatmap::run,
        imported.activity(),
        imported.priority,
        format!("user-{}", imported.user_id),
    )
    .await
}

pub(crate) async fn training_and_fitness<W>(
    ctx: &WorkflowContext<W>,
    imported: &ActivityImportResult,
) -> WorkflowResult<()> {
    let activity = imported.activity();
    let options = options::activity(activity.priority, format!("user-{}", activity.user_id));
    options::run(ctx, DerivedActivities::rebuild_activity_training, activity, options).await?;
    let task = FitnessChange {
        user_id: imported.user_id,
        priority: imported.priority,
    };
    children::run(
        ctx,
        RebuildFitnessFreshness::run,
        task,
        imported.priority,
        format!("user-{}", imported.user_id),
    )
    .await
}
