use super::*;
use crate::pipeline_fixture_tests::database;
use bike_core::entities::strava_workflows;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;

async fn completed(db: &DatabaseConnection, action: Action) -> String {
    let id = format!("completed-{}", action.as_str());
    strava_workflows::ActiveModel {
        id: Set(id.clone()),
        connection_id: Set(1),
        athlete_id: Set(99),
        activity_id: Set(Some(42)),
        action: Set(action.as_str().into()),
        source: Set(json!({})),
        checkpoint: Set(json!({})),
        status: Set("completed".into()),
        error: Set(None),
        accepted_at: Set(Utc::now()),
        completed_at: Set(Some(Utc::now())),
    }
    .insert(db)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn every_operation_processor_accepts_its_completed_work_and_rejects_other_operations() {
    let db = database().await;
    for action in Action::ALL {
        let id = completed(&db, action).await;
        let processor = processor(db.clone(), action);
        assert_eq!(processor.task_type(), action.task_type());
        processor
            .process(1, json!({"data":{"workflow_id":id}}))
            .await
            .unwrap();
        processor
            .process(1, json!({"workflow_id":id}))
            .await
            .unwrap();
        let other = if action == Action::Delete {
            Action::Create
        } else {
            Action::Delete
        };
        let error = super::processor(db.clone(), other)
            .process(1, json!({"data":{"workflow_id":id}}))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("does not match"));
        assert!(processor
            .process(1, json!({"data":{"workflow_id":" "}}))
            .await
            .is_err());
    }
}

#[tokio::test]
async fn historical_generic_jobs_use_the_same_completed_workflow_without_new_jobs() {
    let db = database().await;
    let id = completed(&db, Action::Update).await;
    LegacyWorkflow::new(db)
        .process(1, json!({"data":{"workflow_id":id}}))
        .await
        .unwrap();
}

#[tokio::test]
async fn scheduled_scan_and_retention_remain_separate_from_sync_execution() {
    let db = database().await;
    let scan = StravaReconcile::new(db.clone());
    assert_eq!(scan.task_type(), "strava_reconcile");
    assert_eq!(scan.schedule(), Some("0 0 */6 * * *"));
    scan.process(1, json!({})).await.unwrap();
    let retention = StravaRetention::new(db);
    assert_eq!(retention.task_type(), "strava_retention");
    assert_eq!(retention.schedule(), Some("0 0 * * * *"));
    assert_eq!(StravaSync::new(database().await).schedule(), None);
}
