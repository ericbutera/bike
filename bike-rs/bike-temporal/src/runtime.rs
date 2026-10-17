//! Bike registration is independent of provider workflows and queues.
use crate::{
    activities::{BatchActivities, DerivedActivities, ImportActivities},
    operations::BikeOperations,
    workflows::{
        ActivityArchiveImport, BackfillUserXcTraining, PrepareHeatmap, ProcessActivityImport, RebuildFitnessFreshness,
        RebuildSegmentAnalytics, ReconcileHeatmaps, RegenerateActivitySegments, RegenerateSegmentEfforts,
        RegenerateUserSegments, ReprocessActivityImport, ReprocessArchiveFitActivityImports,
        ReprocessUserActivityImports,
    },
    TASK_QUEUE,
};
use bike_core::object_store::{ObjectOwner, ObjectStore};
use std::{sync::Arc, time::Duration};
use temporalio_client::Client;
use temporalio_sdk::runtime::worker_tuner::{FixedSizeSlotSupplier, TunerHolder};
use temporalio_sdk::{Runtime, Worker, WorkerOptions};

pub async fn connect() -> Result<Client, crate::client::Error> {
    bike_core::jobs::native::connect().await
}

pub async fn ensure_schedule(client: &Client) -> Result<(), crate::client::Error> {
    use temporalio_client::schedules::{
        CreateScheduleOptions, ScheduleAction, ScheduleError, ScheduleIntervalSpec, ScheduleOverlapPolicy, ScheduleSpec,
    };
    let options = CreateScheduleOptions::builder()
        .action(ScheduleAction::start_workflow(
            ReconcileHeatmaps::run,
            0,
            TASK_QUEUE,
            "bike-heatmap-reconciliation",
        ))
        .spec(
            ScheduleSpec::builder()
                .intervals(vec![ScheduleIntervalSpec::from(Duration::from_secs(15))])
                .build(),
        )
        .overlap_policy(ScheduleOverlapPolicy::Skip)
        .build();
    match client.create_schedule("bike-heatmap-reconciliation", options).await {
        Ok(_) => Ok(()),
        Err(ScheduleError::Rpc(status)) if status.code() == tonic::Code::AlreadyExists => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn worker(
    runtime: &Runtime,
    client: Client,
    db: sea_orm::DatabaseConnection,
    uploads_dir: String,
) -> Result<Worker, crate::client::Error> {
    let objects = ObjectStore::filesystem(&uploads_dir, ObjectOwner::ActivityImports);
    let operations = Arc::new(BikeOperations::new(db, objects, uploads_dir));
    let options = WorkerOptions::new(TASK_QUEUE)
        .tuner(
            TunerHolder::builder()
                .workflow_task_slot_supplier(FixedSizeSlotSupplier::new(16))
                .activity_task_slot_supplier(FixedSizeSlotSupplier::new(2))
                .local_activity_task_slot_supplier(FixedSizeSlotSupplier::new(1))
                .nexus_task_slot_supplier(FixedSizeSlotSupplier::new(1))
                .build(),
        )
        .register_workflow::<ProcessActivityImport>()?
        .register_workflow::<ReprocessActivityImport>()?
        .register_workflow::<RebuildFitnessFreshness>()?
        .register_workflow::<RebuildSegmentAnalytics>()?
        .register_workflow::<PrepareHeatmap>()?
        .register_workflow::<RegenerateActivitySegments>()?
        .register_workflow::<RegenerateSegmentEfforts>()?
        .register_workflow::<RegenerateUserSegments>()?
        .register_workflow::<ReprocessUserActivityImports>()?
        .register_workflow::<ReprocessArchiveFitActivityImports>()?
        .register_workflow::<BackfillUserXcTraining>()?
        .register_workflow::<ActivityArchiveImport>()?
        .register_workflow::<ReconcileHeatmaps>()?
        .register_activities(ImportActivities::new(operations.clone()))
        .register_activities(BatchActivities::new(operations.clone()))
        .register_activities(DerivedActivities::new(operations))
        .graceful_shutdown_period(Duration::from_secs(10))
        .build();
    Ok(Worker::new(runtime, client, options)?)
}
