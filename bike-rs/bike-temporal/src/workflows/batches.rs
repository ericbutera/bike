//! Bounded manifest pages, awaited work, and continuation at settled boundaries.
use super::{children, options};
use bike_core::bike_tasks::{
    BatchEntry, BatchInput, BatchManifest, BatchResult, ManifestScope, PageRequest,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::BTreeSet, future::Future};
use temporalio_common::data_converters::TemporalDeserializable;
use temporalio_sdk::{ApplicationFailure, ContinueAsNewOptions, WorkflowContext, WorkflowResult};
use temporalio_workflow::common::{ActivityDefinition, WorkflowDefinition};

const PAGES_PER_RUN: usize = 16;

pub(super) enum BatchStep {
    Complete(BatchResult),
    Continue,
}

pub(super) async fn prepare<W, A>(
    ctx: &WorkflowContext<W>,
    batch: &mut BatchInput,
    definition: A,
    input: A::Input,
    scope: ManifestScope,
) -> WorkflowResult<()>
where
    A: ActivityDefinition<Output = BatchManifest>,
    A::Input: Serialize,
{
    batch.command.validate().map_err(ApplicationFailure::new)?;
    let manifest = match batch.progress.manifest.clone() {
        Some(manifest) => manifest,
        None => {
            options::run(
                ctx,
                definition,
                input,
                options::manifest(
                    batch.command.priority,
                    format!("user-{}", batch.command.user_id),
                ),
            )
            .await?
        }
    };
    if manifest.command_id != batch.command.command_id || manifest.scope != scope {
        return Err(
            ApplicationFailure::new("Manifest does not match the command and selection").into(),
        );
    }
    batch.progress.manifest = Some(manifest);
    batch.page_request().map_err(ApplicationFailure::new)?;
    Ok(())
}

pub(super) async fn run_children<W, A, D>(
    ctx: &WorkflowContext<W>,
    batch: &mut BatchInput,
    page: A,
    child: impl Fn() -> D,
) -> WorkflowResult<BatchStep>
where
    A: temporalio_sdk::activities::ExecutableActivity<Input = PageRequest, Output = Vec<D::Input>>,
    D: WorkflowDefinition + 'static,
    D::Input: BatchEntry + Serialize + DeserializeOwned,
    D::Output: TemporalDeserializable,
{
    run_pages(ctx, batch, page, |input| {
        let definition = child();
        async move { children::run(ctx, definition, input).await.map(|_| ()) }
    })
    .await
}

pub(super) async fn run_pages<W, A, I, F, Fut>(
    ctx: &WorkflowContext<W>,
    batch: &mut BatchInput,
    _page: A,
    mut process: F,
) -> WorkflowResult<BatchStep>
where
    A: temporalio_sdk::activities::ExecutableActivity<Input = PageRequest, Output = Vec<I>>,
    I: BatchEntry + Serialize + DeserializeOwned,
    F: FnMut(I) -> Fut,
    Fut: Future<Output = WorkflowResult<()>>,
{
    for _ in 0..PAGES_PER_RUN {
        let request = batch.page_request().map_err(ApplicationFailure::new)?;
        if request.expected_entries() == 0 {
            return Ok(BatchStep::Complete(batch.result()));
        }
        let activity_options = options::activity(
            batch.command.priority,
            format!("user-{}", batch.command.user_id),
        );
        let entries = options::run(ctx, A::definition(), request.clone(), activity_options).await?;
        validate_page(&request, &entries)?;
        for entry in entries {
            process(entry).await?;
        }
        // Only settled pages become continuation state. Workflow history still
        // records each child/Activity completion during this page.
        batch.progress.settled += request.expected_entries() as u64;
        if batch.progress.settled == request.manifest.entries {
            return Ok(BatchStep::Complete(batch.result()));
        }
        if ctx.continue_as_new_suggested() {
            return Ok(BatchStep::Continue);
        }
    }
    Ok(BatchStep::Continue)
}

fn validate_page<I: BatchEntry>(request: &PageRequest, entries: &[I]) -> WorkflowResult<()> {
    if entries.len() != request.expected_entries() {
        return Err(
            ApplicationFailure::new("Manifest page is truncated or exceeds its bound").into(),
        );
    }
    let prefix = format!("{}:", request.command.command_id);
    let mut seen = BTreeSet::new();
    for entry in entries {
        if !entry.command_id().starts_with(&prefix) || !entry.belongs_to(&request.manifest.scope) {
            return Err(ApplicationFailure::new(
                "Manifest entry belongs to another command or resource",
            )
            .into());
        }
        if !seen.insert(entry.command_id()) {
            return Err(ApplicationFailure::new("Duplicate manifest entry command").into());
        }
    }
    Ok(())
}

pub(super) fn finish_or_continue<W>(
    ctx: &mut WorkflowContext<W>,
    input: <W::Run as WorkflowDefinition>::Input,
    step: BatchStep,
) -> WorkflowResult<BatchResult>
where
    W: temporalio_sdk::workflows::WorkflowImplementation,
{
    options::workflow_result(ctx, Ok(()))?;
    match step {
        BatchStep::Complete(result) => Ok(result),
        BatchStep::Continue => {
            ctx.continue_as_new(input, ContinueAsNewOptions::default())?;
            unreachable!();
        }
    }
}
