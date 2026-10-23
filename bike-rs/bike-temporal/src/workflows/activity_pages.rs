use super::{children, options, RegenerateActivitySegments, ReprocessActivityImport};
use crate::activities::{BatchActivities, DerivedActivities};
use bike_core::bike_tasks::*;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

async fn page<W>(
    ctx: &WorkflowContext<W>,
    page: &UserPage,
    selection: ActivitySelection,
) -> WorkflowResult<Vec<ActivityTask>> {
    options::run(
        ctx,
        BatchActivities::activity_page,
        ActivityPage {
            page: page.clone(),
            selection,
        },
        options::activity(Priority::Background, format!("user-{}", page.user_id)),
    )
    .await
}

async fn reprocess<W>(ctx: &WorkflowContext<W>, task: ActivityTask) -> WorkflowResult<()> {
    let input = options::run(
        ctx,
        BatchActivities::reprocess_input,
        task.clone(),
        options::activity(Priority::Background, format!("user-{}", task.user_id)),
    )
    .await?;
    children::run(
        ctx,
        ReprocessActivityImport::run,
        input,
        Priority::Background,
        format!("user-{}", task.user_id),
    )
    .await
    .map(|_| ())
}

#[workflow]
#[derive(Default)]
pub struct ReprocessUserActivityImports;

#[workflow_methods]
impl ReprocessUserActivityImports {
    #[run(name = "reprocess_user_activity_imports")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut input: UserPage) -> WorkflowResult<()> {
        let tasks = page(
            ctx,
            &input,
            ActivitySelection::Imports {
                archive_fits_only: false,
            },
        )
        .await?;
        let full = tasks.len() == 16;
        for task in tasks {
            input.after_id = task.activity_id;
            reprocess(ctx, task).await?;
        }
        if full {
            return ctx
                .continue_as_new(input, Default::default())
                .map(|never| match never {});
        }
        Ok(())
    }
}

#[workflow]
#[derive(Default)]
pub struct ReprocessArchiveFitActivityImports;

#[workflow_methods]
impl ReprocessArchiveFitActivityImports {
    #[run(name = "reprocess_archive_fit_activity_imports")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut input: UserPage) -> WorkflowResult<()> {
        let tasks = page(
            ctx,
            &input,
            ActivitySelection::Imports {
                archive_fits_only: true,
            },
        )
        .await?;
        let full = tasks.len() == 16;
        for task in tasks {
            input.after_id = task.activity_id;
            reprocess(ctx, task).await?;
        }
        if full {
            return ctx
                .continue_as_new(input, Default::default())
                .map(|never| match never {});
        }
        Ok(())
    }
}

#[workflow]
#[derive(Default)]
pub struct RegenerateUserSegments;

#[workflow_methods]
impl RegenerateUserSegments {
    #[run(name = "regenerate_user_segments")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut input: UserPage) -> WorkflowResult<()> {
        let tasks = page(ctx, &input, ActivitySelection::Cycling).await?;
        let full = tasks.len() == 16;
        for task in tasks {
            input.after_id = task.activity_id;
            children::run(
                ctx,
                RegenerateActivitySegments::run,
                task,
                Priority::Background,
                format!("user-{}", input.user_id),
            )
            .await?;
        }
        if full {
            return ctx
                .continue_as_new(input, Default::default())
                .map(|never| match never {});
        }
        Ok(())
    }
}

#[workflow]
#[derive(Default)]
pub struct BackfillUserXcTraining;

#[workflow_methods]
impl BackfillUserXcTraining {
    #[run(name = "backfill_user_xc_training")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut input: UserPage) -> WorkflowResult<()> {
        let tasks = page(ctx, &input, ActivitySelection::Cycling).await?;
        let full = tasks.len() == 16;
        for task in tasks {
            input.after_id = task.activity_id;
            options::run(
                ctx,
                DerivedActivities::rebuild_activity_training,
                task,
                options::activity(Priority::Background, format!("user-{}", input.user_id)),
            )
            .await?;
        }
        if full {
            return ctx
                .continue_as_new(input, Default::default())
                .map(|never| match never {});
        }
        options::run(
            ctx,
            BatchActivities::finish_training_backfill,
            input.user_id,
            options::activity(Priority::Background, format!("user-{}", input.user_id)),
        )
        .await
    }
}

/// Explicit rollout backfill; only missing or older climb outputs are selected.
#[workflow]
#[derive(Default)]
pub struct BackfillActivityClimbs;

#[workflow_methods]
impl BackfillActivityClimbs {
    #[run(name = "backfill_activity_climbs")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut after_id: i32) -> WorkflowResult<()> {
        let tasks = options::run(
            ctx,
            BatchActivities::pending_climbs,
            after_id,
            options::activity(Priority::Background, "climb-backfill".into()),
        )
        .await?;
        let full = tasks.len() == 16;
        for task in tasks {
            let completed_id = task.activity_id;
            let user_id = task.user_id;
            options::run(
                ctx,
                DerivedActivities::rebuild_activity_climbs,
                task,
                options::activity(Priority::Background, format!("user-{user_id}")),
            )
            .await?;
            after_id = completed_id;
        }
        if full {
            return ctx
                .continue_as_new(after_id, Default::default())
                .map(|never| match never {});
        }
        Ok(())
    }
}

#[workflow]
#[derive(Default)]
pub struct BackfillPersonalAchievements;

#[workflow_methods]
impl BackfillPersonalAchievements {
    #[run(name = "backfill_personal_achievements")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut after_id: i32) -> WorkflowResult<()> {
        let segment_ids = options::run(
            ctx,
            BatchActivities::personal_achievement_segments,
            after_id,
            options::activity(Priority::Background, "personal-achievements-backfill".into()),
        )
        .await?;
        let full = segment_ids.len() == 16;
        for segment_id in segment_ids {
            options::run(
                ctx,
                BatchActivities::rebuild_personal_achievements,
                segment_id,
                options::activity(Priority::Background, format!("segment-{segment_id}")),
            )
            .await?;
            after_id = segment_id;
        }
        if full {
            return ctx
                .continue_as_new(after_id, Default::default())
                .map(|never| match never {});
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bike_core::test_support as fixtures;
    use workflow_test_support::WorkflowMock;

    #[test]
    fn climb_backfill_awaits_each_stage_and_continues_after_a_full_page() {
        let tasks: Vec<_> = (1..=16)
            .map(|activity_id| ActivityTask {
                activity_id,
                priority: Priority::Background,
                ..fixtures::activity_task()
            })
            .collect();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityClimbs>(0);
        mock.activity("pending_climbs", &0_i32, &tasks);
        for task in &tasks {
            mock.activity("rebuild_activity_climbs", task, &());
        }
        let cursor = mock.continued_input::<i32>();
        assert_eq!(cursor, 16);
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityClimbs>(cursor);
        mock.activity("pending_climbs", &cursor, &Vec::<ActivityTask>::new());
        mock.result::<()>();
    }

    #[test]
    fn failed_climb_stops_backfill_before_advancing_to_the_next_activity() {
        let task = fixtures::activity_task();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityClimbs>(0);
        mock.activity("pending_climbs", &0_i32, &vec![task.clone(), task]);
        mock.fail_activity("rebuild_activity_climbs", "climb failed");
        mock.assert_failure_message("climb failed");
    }

    #[test]
    fn personal_achievement_backfill_pages_segments_and_resumes_after_the_completed_page() {
        let ids: Vec<i32> = (1..=16).collect();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillPersonalAchievements>(0);
        mock.activity("personal_achievement_segments", &0_i32, &ids);
        for id in ids {
            mock.activity("rebuild_personal_achievements", &id, &());
        }
        let cursor = mock.continued_input::<i32>();
        assert_eq!(cursor, 16);
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillPersonalAchievements>(cursor);
        mock.activity("personal_achievement_segments", &cursor, &vec![23_i32]);
        mock.activity("rebuild_personal_achievements", &23_i32, &());
        mock.result::<()>();
    }

    #[test]
    fn personal_achievement_backfill_finishes_empty_pages_and_stops_on_failure() {
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillPersonalAchievements>(0);
        mock.activity("personal_achievement_segments", &0_i32, &Vec::<i32>::new());
        mock.result::<()>();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillPersonalAchievements>(0);
        mock.activity("personal_achievement_segments", &0_i32, &vec![1_i32, 2]);
        mock.fail_activity("rebuild_personal_achievements", "personal achievements failed");
        mock.assert_failure_message("personal achievements failed");
    }
}

#[workflow]
#[derive(Default)]
pub struct BackfillOverallAchievements;

#[workflow_methods]
impl BackfillOverallAchievements {
    #[run(name = "backfill_overall_achievements")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut after_id: i32) -> WorkflowResult<()> {
        let segment_ids = options::run(
            ctx,
            BatchActivities::overall_achievement_segments,
            after_id,
            options::activity(Priority::Background, "overall-achievements-backfill".into()),
        )
        .await?;
        let full = segment_ids.len() == 16;
        for segment_id in segment_ids {
            options::run(
                ctx,
                BatchActivities::rebuild_overall_achievements,
                segment_id,
                options::activity(Priority::Background, format!("segment-{segment_id}")),
            )
            .await?;
            after_id = segment_id;
        }
        if full {
            return ctx
                .continue_as_new(after_id, Default::default())
                .map(|never| match never {});
        }
        Ok(())
    }
}

#[cfg(test)]
mod overall_tests {
    use super::*;
    use workflow_test_support::WorkflowMock;

    #[test]
    fn a_full_page_continues_after_all_replacements_complete() {
        let ids: Vec<i32> = (1..=16).collect();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillOverallAchievements>(0);
        mock.activity("overall_achievement_segments", &0_i32, &ids);
        for id in ids {
            mock.activity("rebuild_overall_achievements", &id, &());
        }
        assert_eq!(mock.continued_input::<i32>(), 16);
    }

    #[test]
    fn a_partial_page_resumes_from_the_supplied_cursor() {
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillOverallAchievements>(16);
        mock.activity("overall_achievement_segments", &16_i32, &vec![23_i32]);
        mock.activity("rebuild_overall_achievements", &23_i32, &());
        mock.result::<()>();
    }

    #[test]
    fn an_empty_page_finishes_the_backfill() {
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillOverallAchievements>(0);
        mock.activity("overall_achievement_segments", &0_i32, &Vec::<i32>::new());
        mock.result::<()>();
    }

    #[test]
    fn a_failed_replacement_stops_the_backfill() {
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillOverallAchievements>(0);
        mock.activity("overall_achievement_segments", &0_i32, &vec![1_i32, 2]);
        mock.fail_activity("rebuild_overall_achievements", "overall achievements failed");
        mock.assert_failure_message("overall achievements failed");
    }
}
