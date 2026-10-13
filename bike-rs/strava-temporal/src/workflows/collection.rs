use super::options::run;
use super::{options, UpsertActivity};
use crate::activities::{AcceptChild, ListActivityPage};
use bike_core::strava::{events::StravaOperation, native::*};
use temporalio_sdk::{ChildWorkflowOptions, WorkflowContext, WorkflowResult};

pub(super) enum CollectionStep {
    Completed(CollectionResult),
    Continue(bike_core::strava::events::StravaEvent),
}

pub(super) async fn upsert_pages<W>(
    ctx: &mut WorkflowContext<W>,
    mut input: CollectionInput,
) -> WorkflowResult<CollectionStep> {
    input.event.window = Some(input.window.clone());
    let progress = input.event.progress.clone();
    let mut result = CollectionResult {
        event: input.event.clone(),
        imported: progress.as_ref().map_or(0, |p| p.imported),
        unchanged: progress.as_ref().map_or(0, |p| p.unchanged),
        last_started_at: progress.as_ref().and_then(|p| p.last_started_at),
    };
    let mut page = progress.map_or(1, |p| p.next_page);
    let mut pages = 0;
    loop {
        let listed = run(
            ctx,
            ListActivityPage,
            PageInput {
                collection: input.clone(),
                page,
            },
        )
        .await?;
        for id in listed.activity_ids {
            let child = input.event.for_activity(id, StravaOperation::Upsert);
            let child = run(
                ctx,
                AcceptChild,
                ChildEvent {
                    parent: input.event.clone(),
                    child,
                },
            )
            .await?;
            let options = ChildWorkflowOptions::builder()
                .workflow_id(child.workflow_id())
                .search_attributes(crate::client::search_attributes(&child))
                .build();
            let changed =
                options::execute_child_workflow(ctx, UpsertActivity::run, child, options).await?;
            if changed.status == ActivityStatus::Applied {
                result.imported += 1;
            } else {
                result.unchanged += 1;
            }
        }
        result.last_started_at = result
            .last_started_at
            .into_iter()
            .chain(listed.latest_started_at)
            .max();
        let Some(next) = listed.next_page else {
            return Ok(CollectionStep::Completed(result));
        };
        page = next;
        pages += 1;
        if pages >= 25 || ctx.continue_as_new_suggested() {
            result.event.progress = Some(CollectionProgress {
                next_page: page,
                imported: result.imported,
                unchanged: result.unchanged,
                last_started_at: result.last_started_at,
            });
            return Ok(CollectionStep::Continue(result.event));
        }
    }
}
