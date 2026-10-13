use super::*;
use crate::background_jobs::{
    entities::{background_tasks, pipeline_runs, pipeline_subjects, pipeline_tasks, task_attempts},
    pipeline::PipelineContext,
};
use crate::strava::{client::StravaApiClient, objects::ObjectStore, paths::WorkflowPaths};
use crate::{
    config::Config,
    entities::{activity_import_locks, integration_events, strava_connections, strava_workflows},
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, Schema, Set,
};
use serde_json::json;

struct Fixture {
    db: DatabaseConnection,
    row: strava_workflows::Model,
    connection: strava_connections::Model,
    store: ObjectStore,
    _files: tempfile::TempDir,
}

impl Fixture {
    async fn new() -> Self {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let schema = Schema::new(db.get_database_backend());
        for table in [
            schema.create_table_from_entity(background_tasks::Entity),
            schema.create_table_from_entity(pipeline_runs::Entity),
            schema.create_table_from_entity(pipeline_tasks::Entity),
            schema.create_table_from_entity(pipeline_subjects::Entity),
            schema.create_table_from_entity(task_attempts::Entity),
            schema.create_table_from_entity(strava_connections::Entity),
            schema.create_table_from_entity(strava_workflows::Entity),
            schema.create_table_from_entity(integration_events::Entity),
            schema.create_table_from_entity(activity_import_locks::Entity),
        ] {
            db.execute(&table).await.unwrap();
        }
        let connection = strava_connections::ActiveModel {
            id: Set(1),
            user_id: Set(7),
            athlete_id: Set(99),
            access_token: Set("fixture-token".into()),
            refresh_token: Set("fixture-refresh".into()),
            scopes: Set("activity:read_all".into()),
            expires_at: Set(Utc::now() + chrono::Duration::hours(1)),
            last_sync_status: Set("queued".into()),
            last_sync_imported_count: Set(0),
            last_sync_duplicate_count: Set(0),
            last_sync_failed_count: Set(0),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let source = Source {
            revoke_remote: false,
            generation: connection.created_at,
            event_time: Utc::now().timestamp(),
            parent: None,
            activity: None,
            listing_page: None,
            raw: None,
            after: None,
        };
        inbox::accept(&db, &connection, "parent", Action::Sync, None, source)
            .await
            .unwrap();
        let row = strava_workflows::Model::load(&db, "parent")
            .await
            .unwrap()
            .unwrap();
        let files = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(files.path());
        Self {
            db,
            row,
            connection,
            store,
            _files: files,
        }
    }

    fn run(&self) -> Run<'_> {
        Run {
            db: &self.db,
            row: self.row.clone(),
            action: self.row.action.parse().unwrap(),
            source: Source::from_value(self.row.source.clone()).unwrap(),
            connection: self.connection.clone(),
            client: StravaApiClient::new(&self.db, Config::get()).unwrap(),
            store: self.store.clone(),
        }
    }

    fn child_source(&self) -> Source {
        Source {
            revoke_remote: false,
            generation: self.connection.created_at,
            event_time: self.row.accepted_at.timestamp(),
            parent: Some(self.row.id.clone()),
            activity: None,
            listing_page: None,
            raw: None,
            after: None,
        }
    }

    async fn seed_discovery(&mut self) {
        let listing = self
            .store
            .transform(
                &WorkflowPaths::new(&self.row).raw("", "pages/1"),
                &json!([{"id":42,"start_date":self.row.accepted_at.to_rfc3339()}]),
            )
            .await
            .unwrap();
        let output = self
            .store
            .transform(
                &WorkflowPaths::new(&self.row).transform("discover", &self.row.id),
                &json!([{"observed_at":self.row.accepted_at,"raw":listing}]),
            )
            .await
            .unwrap();
        let mut checkpoint = self.row.checkpoint.clone();
        checkpoint[format!("discover:v1:{}", self.row.id)] = serde_json::to_value(output).unwrap();
        self.row = self.row.checkpoint(&self.db, checkpoint).await.unwrap();
    }

    async fn queued_child(&self) -> strava_workflows::Model {
        self.run()
            .child_activity("child", Action::Upsert, 42, self.child_source())
            .await
            .unwrap()
    }

    async fn task_status(&self, child: &strava_workflows::Model, status: &str) {
        let task = background_tasks::Model::latest_strava_workflow_task(&self.db, &child.id)
            .await
            .unwrap()
            .unwrap();
        let mut active: background_tasks::ActiveModel = task.into();
        active.status = Set(status.into());
        active.error = Set(Some("fixture child outcome".into()));
        active.update(&self.db).await.unwrap();
    }

    async fn complete_child(&self, child: strava_workflows::Model) {
        let output = self.store.transform(&WorkflowPaths::new(&child).transform("apply", "fixture"),
            &json!({"outcome":"applied","hash":null,"activity_id":42,"import_id":null,"rebuild":{"import_ids":[],"segment_ids":[],"dirty_from":null}})).await.unwrap();
        let mut checkpoint = child.checkpoint.clone();
        checkpoint["apply:v1:fixture"] = serde_json::to_value(output).unwrap();
        let child = child.checkpoint(&self.db, checkpoint).await.unwrap();
        child.finish(&self.db, None).await.unwrap();
        self.task_status(&child, "completed").await;
    }
}

#[tokio::test]
async fn child_acceptance_queues_one_typed_task_with_its_pipeline_parent() {
    let fixture = Fixture::new().await;
    let origin = PipelineContext::received("strava_sync", Some("request".into())).for_task(123);
    let child = PipelineContext::scope(Some(origin.clone()), fixture.queued_child()).await;
    assert!(!fixture.run().child_completed(&child).await.unwrap());
    fixture.queued_child().await;
    assert_eq!(
        background_tasks::Entity::find()
            .count(&fixture.db)
            .await
            .unwrap(),
        1
    );
    let task = background_tasks::Model::latest_strava_workflow_task(&fixture.db, &child.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task.task_type, Action::Upsert.task_type());
    assert_eq!(task.payload["data"]["workflow_id"], child.id);
    let context = PipelineContext::from_payload(&task.payload)
        .unwrap()
        .unwrap();
    assert_eq!(context.parent_task_id, Some(123));
    assert_eq!(context.run_id, origin.run_id);
    assert_eq!(task.payload["strava"]["parent_workflow_id"], "parent");
    assert!(
        background_tasks::Model::active_strava_workflow_ids(&fixture.db)
            .await
            .unwrap()
            .contains(&child.id)
    );
}

#[tokio::test]
async fn sync_waits_for_real_child_tasks_then_updates_counts_and_cursor() {
    let mut fixture = Fixture::new().await;
    fixture.seed_discovery().await;
    assert_eq!(
        fixture.run().collection().await.unwrap(),
        Execution::WaitingForChildren
    );
    let child = strava_workflows::Entity::find()
        .all(&fixture.db)
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.source["parent"] == "parent")
        .unwrap();
    let connection = strava_connections::Entity::find_by_id(1)
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(connection.last_sync_status, "running");
    assert!(connection.last_synced_activity_started_at.is_none());
    wait_for_children(&fixture.db, "parent").await.unwrap();
    assert!(strava_workflows::Model::waiting(&fixture.db, 1)
        .await
        .unwrap()
        .is_none());
    let continuation = background_tasks::Model::latest_strava_workflow_task(&fixture.db, "parent")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(continuation.task_type, Action::Sync.task_type());
    assert!(continuation.scheduled_for.unwrap() > Utc::now());
    fixture.complete_child(child).await;
    assert_eq!(
        fixture.run().collection().await.unwrap(),
        Execution::Completed
    );
    let connection = strava_connections::Entity::find_by_id(1)
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(connection.last_sync_status, "succeeded");
    assert_eq!(connection.last_sync_imported_count, 1);
    assert_eq!(
        connection.last_synced_activity_started_at,
        Some(fixture.row.accepted_at)
    );
    fixture.run().collection().await.unwrap();
    let history = integration_events::Entity::find()
        .filter(integration_events::Column::EventType.eq("sync.completed"))
        .all(&fixture.db)
        .await
        .unwrap();
    assert_eq!(
        history.len(),
        1,
        "a cached collection retry retains one completion event"
    );
    assert_eq!(history[0].payload.as_ref().unwrap()["imported_count"], 1);
}

#[tokio::test]
async fn failed_or_canceled_children_never_report_completion() {
    for status in ["failed", "canceled", "completed"] {
        let fixture = Fixture::new().await;
        let child = fixture.queued_child().await;
        fixture.task_status(&child, status).await;
        assert!(fixture
            .run()
            .child_completed(&child)
            .await
            .unwrap_err()
            .message
            .contains("did not complete"));
    }
    let fixture = Fixture::new().await;
    let child = fixture.queued_child().await;
    let completed = child.finish(&fixture.db, None).await.unwrap();
    fixture.task_status(&completed, "canceled").await;
    assert!(fixture.run().child_completed(&completed).await.is_err());
}

#[tokio::test]
async fn legacy_inline_children_receive_a_worker_handoff_when_needed() {
    let fixture = Fixture::new().await;
    inbox::accept(
        &fixture.db,
        &fixture.connection,
        "child",
        Action::Upsert,
        Some(42),
        fixture.child_source(),
    )
    .await
    .unwrap();
    assert_eq!(
        background_tasks::Entity::find()
            .count(&fixture.db)
            .await
            .unwrap(),
        0
    );
    let child = fixture.queued_child().await;
    assert_eq!(
        background_tasks::Entity::find()
            .count(&fixture.db)
            .await
            .unwrap(),
        1
    );
    let completed = child.finish(&fixture.db, None).await.unwrap();
    assert!(!fixture.run().child_completed(&completed).await.unwrap());
    fixture.task_status(&completed, "completed").await;
    assert!(fixture.run().child_completed(&completed).await.unwrap());
    fixture.queued_child().await;
    assert_eq!(
        background_tasks::Entity::find()
            .count(&fixture.db)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn parent_failure_keeps_the_sync_cursor_unchanged() {
    let mut fixture = Fixture::new().await;
    fixture.seed_discovery().await;
    assert_eq!(
        fixture.run().collection().await.unwrap(),
        Execution::WaitingForChildren
    );
    let child = strava_workflows::Entity::find()
        .all(&fixture.db)
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.source["parent"] == "parent")
        .unwrap();
    fixture.task_status(&child, "failed").await;
    assert!(fixture.run().collection().await.is_err());
    let connection = strava_connections::Entity::find_by_id(1)
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(connection.last_sync_status, "failed");
    assert!(connection.last_synced_activity_started_at.is_none());
    assert!(connection.last_sync_finished_at.is_none());
}
