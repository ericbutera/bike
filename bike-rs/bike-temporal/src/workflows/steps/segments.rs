use super::super::{batches, options};
use crate::activities::SegmentActivities;
use bike_core::bike_tasks::{
    validate_revision, ActivitySegmentsInput, ActivitySegmentsResult, BatchInput, ManifestScope,
    SegmentEffortsInput, SegmentEffortsSelection, SegmentSelection,
};
use temporalio_sdk::{ApplicationFailure, WorkflowContext, WorkflowResult};

pub(crate) async fn match_activity<W>(
    ctx: &WorkflowContext<W>,
    command: ActivitySegmentsInput,
) -> WorkflowResult<ActivitySegmentsResult> {
    command
        .command
        .validate()
        .map_err(ApplicationFailure::new)?;
    validate_revision(command.expected_revision).map_err(ApplicationFailure::new)?;
    if let SegmentSelection::One {
        expected_revision, ..
    } = command.selection
    {
        validate_revision(expected_revision).map_err(ApplicationFailure::new)?;
    }
    let options = options::activity(
        command.command.priority,
        format!("user-{}", command.command.user_id),
    );
    options::run(
        ctx,
        SegmentActivities::match_activity_segments,
        command,
        options,
    )
    .await
}

pub(crate) async fn prepare_user<W>(
    ctx: &WorkflowContext<W>,
    batch: &mut BatchInput,
) -> WorkflowResult<()> {
    let command = batch.command.clone();
    let expected = ManifestScope::UserSegments {
        user_id: command.user_id,
    };
    batches::prepare(
        ctx,
        batch,
        SegmentActivities::prepare_user_segments,
        command,
        expected,
    )
    .await
}

pub(crate) async fn prepare_segment<W>(
    ctx: &WorkflowContext<W>,
    input: &mut SegmentEffortsInput,
) -> WorkflowResult<()> {
    validate_revision(input.expected_revision).map_err(ApplicationFailure::new)?;
    let selection = SegmentEffortsSelection {
        command: input.batch.command.clone(),
        segment_id: input.segment_id,
        expected_revision: input.expected_revision,
    };
    let expected = ManifestScope::Segment {
        segment_id: input.segment_id,
        expected_revision: input.expected_revision,
    };
    batches::prepare(
        ctx,
        &mut input.batch,
        SegmentActivities::prepare_segment_efforts,
        selection,
        expected,
    )
    .await
}
