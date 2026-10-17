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
    let mut page = progress.as_ref().map_or(1, |p| p.next_page);
    let mut pending_page = progress.and_then(|p| p.pending_page);
    let mut pages = 0;
    loop {
        let mut listed = match pending_page.take() {
            Some(listed) => listed,
            None => {
                run(
                    ctx,
                    ListActivityPage,
                    PageInput {
                        collection: input.clone(),
                        page,
                    },
                )
                .await?
            }
        };
        result.last_started_at = result.last_started_at.into_iter().chain(listed.latest_started_at).max();
        let mut remaining = std::mem::take(&mut listed.activity_ids).into_iter();
        while let Some(id) = remaining.next() {
            let changed = upsert_activity(ctx, &input.event, id).await?;
            if changed.status == ActivityStatus::Applied {
                result.imported += 1;
            } else {
                result.unchanged += 1;
            }
            let history_limit = ctx.continue_as_new_suggested() && remaining.len() > 0;
            if history_limit && ctx.patched("strava-partial-page-continuation-v1") {
                listed.activity_ids = remaining.collect();
                return Ok(continue_collection(result, page, Some(listed)));
            }
        }
        let Some(next) = listed.next_page else {
            return Ok(CollectionStep::Completed(result));
        };
        page = next;
        pages += 1;
        if pages >= 25 || ctx.continue_as_new_suggested() {
            return Ok(continue_collection(result, page, None));
        }
    }
}

async fn upsert_activity<W>(
    ctx: &mut WorkflowContext<W>,
    event: &bike_core::strava::events::StravaEvent,
    id: i64,
) -> WorkflowResult<ActivityChange> {
    let child = run(
        ctx,
        AcceptChild,
        ChildEvent {
            parent: event.clone(),
            child: event.for_activity(id, StravaOperation::Upsert),
        },
    )
    .await?;
    let options = ChildWorkflowOptions::builder()
        .workflow_id(child.workflow_id())
        .search_attributes(crate::client::search_attributes(&child))
        .build();
    options::execute_child_workflow(ctx, UpsertActivity::run, child, options).await
}

fn continue_collection(
    mut result: CollectionResult,
    next_page: i32,
    pending_page: Option<ActivityPage>,
) -> CollectionStep {
    result.event.progress = Some(CollectionProgress {
        next_page,
        pending_page,
        imported: result.imported,
        unchanged: result.unchanged,
        last_started_at: result.last_started_at,
    });
    CollectionStep::Continue(result.event)
}
