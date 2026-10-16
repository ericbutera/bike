use super::{
    super::{batches, options},
    derived,
};
use crate::activities::TrainingActivities;
use bike_core::bike_tasks::{
    validate_revision, BatchInput, BatchResult, ManifestScope, TrainingActivityInput,
    TrainingCompletion,
};
use temporalio_sdk::{ApplicationFailure, WorkflowContext, WorkflowResult};

pub(crate) async fn prepare<W>(
    ctx: &WorkflowContext<W>,
    batch: &mut BatchInput,
) -> WorkflowResult<()> {
    let command = batch.command.clone();
    let expected = ManifestScope::Training {
        user_id: command.user_id,
    };
    batches::prepare(
        ctx,
        batch,
        TrainingActivities::prepare_training_backfill,
        command,
        expected,
    )
    .await
}

pub(crate) async fn rebuild_activity<W>(
    ctx: &WorkflowContext<W>,
    command: TrainingActivityInput,
) -> WorkflowResult<()> {
    command
        .command
        .validate()
        .map_err(ApplicationFailure::new)?;
    validate_revision(command.expected_revision).map_err(ApplicationFailure::new)?;
    let options = options::activity(
        command.command.priority,
        format!("user-{}", command.command.user_id),
    );
    let result = options::run(
        ctx,
        TrainingActivities::rebuild_activity_training,
        command,
        options,
    )
    .await?;
    if let Some(change) = result.fitness {
        derived::fitness(ctx, &change).await?;
    }
    Ok(())
}

pub(crate) async fn complete<W>(
    ctx: &WorkflowContext<W>,
    batch: &BatchInput,
    result: &BatchResult,
) -> WorkflowResult<()> {
    let completion = TrainingCompletion {
        command: batch.command.clone(),
        result: result.clone(),
    };
    let options = options::activity(
        batch.command.priority,
        format!("user-{}", batch.command.user_id),
    );
    options::run(
        ctx,
        TrainingActivities::complete_training_backfill,
        completion,
        options,
    )
    .await
}
