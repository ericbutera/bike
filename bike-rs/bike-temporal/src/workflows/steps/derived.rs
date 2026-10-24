use super::super::{children, options, PrepareHeatmap, RebuildFitnessFreshness, RebuildSegmentAnalytics};
use crate::activities::DerivedActivities;
use bike_core::bike_tasks::{ActivityImportResult, ActivityTask, FitnessChange, SegmentChange};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

/// Imports and reprocessing await the same analytics and report work after save.
pub(crate) async fn rebuild<W>(ctx: &WorkflowContext<W>, imported: &ActivityImportResult) -> WorkflowResult<()> {
    let existing = async {
        segments(ctx, imported).await?;
        achievements(ctx, imported).await?;
        heatmap(ctx, imported).await?;
        training_and_fitness(ctx, imported).await
    };
    let with_climbs = ctx.patched("activity-climbs-v1");
    let with_cards = ctx.patched("activity-cards-v1");
    let derived = async {
        if with_climbs {
            let (derived, climbs) = futures::join!(existing, activity_climbs(ctx, imported));
            derived?;
            climbs
        } else {
            existing.await
        }
    };
    if with_cards {
        let (derived, card) = futures::join!(derived, activity_card(ctx, imported));
        derived?;
        card
    } else {
        derived.await
    }
}

async fn activity_card<W>(ctx: &WorkflowContext<W>, imported: &ActivityImportResult) -> WorkflowResult<()> {
    options::run(
        ctx,
        DerivedActivities::rebuild_activity_card,
        imported.activity(),
        options::activity(imported.priority, format!("user-{}", imported.user_id)),
    )
    .await
}

async fn activity_climbs<W>(ctx: &WorkflowContext<W>, imported: &ActivityImportResult) -> WorkflowResult<()> {
    options::run(
        ctx,
        DerivedActivities::rebuild_activity_climbs,
        imported.activity(),
        options::activity(imported.priority, format!("user-{}", imported.user_id)),
    )
    .await
}

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
