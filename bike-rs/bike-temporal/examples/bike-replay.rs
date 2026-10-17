//! Read-only replay for the explicit Bike workflow sequence. Drain pre-simplification runs before rollout.
use bike_temporal::{client::Error, runtime::connect, workflows::*, TASK_QUEUE};
use temporalio_client::{WorkflowFetchHistoryOptions, WorkflowHistory};
use temporalio_sdk::workflow_replayer::{WorkflowReplayer, WorkflowReplayerOptions};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().ok_or("Use --file history.json or --workflow workflow-id")?;
    let value = args.next().ok_or("A history path or workflow ID is required")?;
    if args.next().is_some() {
        return Err("Expected exactly a mode and value".into());
    }
    let history = match mode.as_str() {
        "--file" => WorkflowHistory::from_json(&tokio::fs::read(&value).await?)?,
        "--workflow" => connect()
            .await?
            .get_workflow_handle::<RebuildFitnessFreshness>(value.clone())
            .fetch_history(WorkflowFetchHistoryOptions::default()),
        _ => return Err("Use --file or --workflow".into()),
    };
    let options = WorkflowReplayerOptions::new()
        .namespace("bike")
        .task_queue(TASK_QUEUE)
        .register_workflow::<ProcessActivityImport>()?
        .register_workflow::<ReprocessActivityImport>()?
        .register_workflow::<RebuildFitnessFreshness>()?
        .register_workflow::<RebuildSegmentAnalytics>()?
        .register_workflow::<PrepareHeatmap>()?
        .build();
    WorkflowReplayer::new(options)?.replay_workflow(history).await?;
    println!("Native worker replay passed: {value}");
    Ok(())
}
