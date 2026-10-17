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
