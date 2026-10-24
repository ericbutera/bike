//! Bike registration is independent of provider workflows and queues.
use crate::{
    activities::{BatchActivities, DerivedActivities, ImportActivities},
    operations::BikeOperations,
    workflows::{
        ActivityArchiveImport, BackfillActivityCards, BackfillActivityClimbs, BackfillEarnedAwards,
        BackfillOverallAchievements, BackfillPersonalAchievements, BackfillUserXcTraining, PrepareHeatmap,
        ProcessActivityImport, RebuildFitnessFreshness, RebuildSegmentAnalytics, ReconcileHeatmaps,
        RegenerateActivitySegments, RegenerateSegmentEfforts, RegenerateUserSegments, ReprocessActivityImport,
        ReprocessArchiveFitActivityImports, ReprocessUserActivityImports,
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

pub fn worker(
    runtime: &Runtime,
    client: Client,
    db: sea_orm::DatabaseConnection,
    uploads_dir: String,
) -> Result<Worker, crate::client::Error> {
    let objects = ObjectStore::filesystem(&uploads_dir, ObjectOwner::ActivityImports);
    let metadata = crate::workflows::MetadataActivities::new(db.clone());
    let recordings = crate::workflows::RecordingActivities::new(db.clone(), objects.clone());
    let operations = Arc::new(BikeOperations::new(db, objects));
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
        .register_workflow::<BackfillActivityClimbs>()?
        .register_workflow::<BackfillActivityCards>()?
        .register_workflow::<BackfillPersonalAchievements>()?
        .register_workflow::<BackfillOverallAchievements>()?
        .register_workflow::<BackfillEarnedAwards>()?
        .register_workflow::<crate::workflows::BackfillActivityMetadata>()?
        .register_activities(metadata)
        .register_workflow::<crate::workflows::BackfillActivityRecordings>()?
        .register_activities(recordings)
        .register_activities(ImportActivities::new(operations.clone()))
        .register_activities(BatchActivities::new(operations.clone()))
        .register_activities(DerivedActivities::new(operations))
        .graceful_shutdown_period(Duration::from_secs(10))
        .build();
    Ok(Worker::new(runtime, client, options)?)
}
