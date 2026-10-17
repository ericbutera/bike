//! Read-only replay of a Strava workflow history with the native SDK worker.
use strava_temporal::{
    client::{connect, Error, TemporalConfig},
    workflows::*,
};
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
    let config = TemporalConfig::from_env();
    let history = match mode.as_str() {
        "--file" => WorkflowHistory::from_json(&tokio::fs::read(&value).await?)?,
        "--workflow" => connect(&config)
            .await?
            .get_workflow_handle::<UpsertActivity>(value.clone())
            .fetch_history(WorkflowFetchHistoryOptions::default()),
        _ => return Err("Use --file or --workflow".into()),
    };
    let options = WorkflowReplayerOptions::new()
        .namespace(config.namespace)
        .task_queue(config.task_queue)
        .register_workflow::<CreateActivity>()?
        .register_workflow::<UpdateActivity>()?
        .register_workflow::<UpsertActivity>()?
        .register_workflow::<ReconcileActivity>()?
        .register_workflow::<DeleteActivity>()?
        .register_workflow::<SyncActivities>()?
        .register_workflow::<BackfillActivities>()?
        .register_workflow::<DisconnectStrava>()?
        .register_workflow::<ScheduledSync>()?
        .build();
    WorkflowReplayer::new(options)?.replay_workflow(history).await?;
    println!("Native worker replay passed: {value}");
    Ok(())
}
