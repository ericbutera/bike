use super::*;
use bike_core::background_jobs::entities::{background_tasks, pipeline_runs, processor_registry};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

async fn seed_expired_evidence(db: &sea_orm::DatabaseConnection) {
    let old = Utc::now() - chrono::Duration::days(31);
    pipeline_runs::ActiveModel {
        id: Set("closed".into()),
        pipeline_started_at: Set(old),
        accepted_at: Set(old),
        entrypoint: Set("fixture".into()),
        available_at: Set(Some(old)),
        request_id: Set(None),
        trace_id: Set(None),
        gateway_history: Set(None),
    }
    .insert(db)
    .await
    .unwrap();
}

#[tokio::test]
async fn pipeline_sampler_refreshes_queue_health_and_expires_closed_evidence() {
    let db = pipeline_fixture_tests::database().await;
    background_tasks::Model::enqueue(
        &db,
        "rebuild_fitness_freshness".into(),
        serde_json::json!({"data":{"user_id":7}}),
        None,
        3,
    )
    .await
    .unwrap();
    seed_expired_evidence(&db).await;
    let queue = IntGauge::new("sampler_queue_fixture", "fixture").unwrap();
    let worker = start_queue_diagnostics(
        db.clone(),
        queue.clone(),
        Arc::new(WorkerMetrics::new("sampler_fixture")),
        vec!["rebuild_fitness_freshness".into()],
    )
    .await
    .unwrap();
    assert!(
        processor_registry::Entity::find_by_id("rebuild_fitness_freshness")
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while queue.get() != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(3601)).await;
    tokio::time::resume();
    tokio::time::timeout(Duration::from_secs(5), async {
        while pipeline_runs::Entity::find_by_id("closed")
            .one(&db)
            .await
            .unwrap()
            .is_some()
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    // Deletion becomes visible before retention returns. Observe the next poll
    // to prove the complete cycle finished and sampling remains live.
    queue.set(-1);
    tokio::time::timeout(Duration::from_secs(20), async {
        while queue.get() != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(queue.get(), 1);
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
}
