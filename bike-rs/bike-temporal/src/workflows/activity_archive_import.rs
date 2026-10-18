use super::{children, options, ProcessActivityImport};
use crate::activities::BatchActivities;
pub use bike_core::bike_tasks::ArchiveInput;
use bike_core::bike_tasks::Priority;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct ActivityArchiveImport;

#[workflow_methods]
impl ActivityArchiveImport {
    #[run(name = "activity_archive_import")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, input: ArchiveInput) -> WorkflowResult<()> {
        let options = options::activity(Priority::Background, format!("archive-{}", input.job_id));
        let mut cursor = match input.cursor {
            Some(cursor) => cursor,
            None => options::run(ctx, BatchActivities::prepare_archive, input.job_id, options.clone()).await?,
        };
        let page = options::run(ctx, BatchActivities::archive_page, cursor.clone(), options.clone()).await?;
        for source in page.sources {
            let user_id = source.user_id;
            children::run(
                ctx,
                ProcessActivityImport::run,
                source,
                Priority::Background,
                format!("user-{user_id}"),
            )
            .await?;
        }
        if !page.done {
            cursor.offset = page.next_offset;
            return ctx
                .continue_as_new(
                    ArchiveInput {
                        job_id: input.job_id,
                        user_id: input.user_id,
                        cursor: Some(cursor),
                    },
                    Default::default(),
                )
                .map(|never| match never {});
        }
        options::run(ctx, BatchActivities::finish_archive, input.job_id, options).await
    }
}
