use super::{queue_archive_page, tests::*};
use crate::{
    background_jobs::{
        batches,
        entities::{background_tasks, pipeline_outputs, worker_batches},
        pipeline::PipelineContext,
    },
    entities::{activity_archive_import_jobs, activity_import_locks, activity_imports},
};
use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, Schema, Set,
};
use serde_json::json;

struct ArchiveFixture {
    db: DatabaseConnection,
    uploads: String,
    path: std::path::PathBuf,
    job: i32,
    parent: i32,
    lock: i32,
    origin: PipelineContext,
}
impl ArchiveFixture {
    async fn new() -> Self {
        let db = test_db().await;
        db.execute(
            &Schema::new(db.get_database_backend())
                .create_table_from_entity(activity_import_locks::Entity),
        )
        .await
        .unwrap();
        let path = archive_sources();
        let job = activity_archive_import_jobs::ActiveModel {
            user_id: Set(1),
            user_storage_key: Set("fixture".into()),
            archive_url: Set("https://example.test/archive.zip".into()),
            status: Set("running".into()),
            total_entries: Set(0),
            supported_entry_count: Set(0),
            imported_count: Set(0),
            duplicate_count: Set(0),
            skipped_unsupported_count: Set(0),
            failed_count: Set(0),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let lock = activity_import_locks::ActiveModel {
            user_id: Set(1),
            source: Set("archive_import".into()),
            stage: Set("running".into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let origin = PipelineContext::received("archive_import", None);
        let parent = PipelineContext::scope(
            Some(origin.clone()),
            background_tasks::Model::enqueue(
                &db,
                "activity_archive_import".into(),
                json!({"data":{"job_id":job.id}}),
                None,
                3,
            ),
        )
        .await
        .unwrap();
        PipelineContext::scope(Some(origin.clone().for_task(parent.id)), batches::start(
            &db, parent.id, 1, "archive", json!({"job_id":job.id,"archive_path":path,"resolved_url":job.archive_url,"lock_id":lock.id}),
        )).await.unwrap();
        Self {
            db,
            uploads: test_uploads_dir(),
            path,
            job: job.id,
            parent: parent.id,
            lock: lock.id,
            origin,
        }
    }
    async fn page(&self) {
        PipelineContext::scope(
            Some(self.origin.clone().for_task(self.parent)),
            queue_archive_page(
                &self.db,
                &self.uploads,
                self.parent,
                self.job,
                Some(self.parent),
            ),
        )
        .await
        .unwrap();
    }
    async fn finish_children(&self, failed_import: bool) {
        // This barrier fixture supplies committed domain outcomes; parser behavior
        // is covered by the archive import fixtures in the owning module.
        activity_imports::Entity::update_many()
            .col_expr(activity_imports::Column::Status, Expr::value("processed"))
            .exec(&self.db)
            .await
            .unwrap();
        if failed_import {
            activity_imports::Entity::update_many()
                .col_expr(activity_imports::Column::Status, Expr::value("failed"))
                .filter(activity_imports::Column::Id.eq(1))
                .exec(&self.db)
                .await
                .unwrap();
        }
        for task in background_tasks::Entity::find()
            .all(&self.db)
            .await
            .unwrap()
        {
            task.claim(&self.db)
                .await
                .unwrap()
                .unwrap()
                .mark_completed(&self.db)
                .await
                .unwrap();
        }
        batches::reconcile(&self.db).await.unwrap();
    }
}

fn archive_sources() -> std::path::PathBuf {
    let entries = (0..17)
        .map(|id| {
            (
                format!("ride-{id}.gpx"),
                format!("<gpx><name>Fixture {id}</name></gpx>").into_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let borrowed = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    write_test_archive(&borrowed)
}

async fn assert_archive_barrier(fixture: &ArchiveFixture, failed_import: bool) {
    let batch = worker_batches::Entity::find_by_id(fixture.parent)
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        batch.status,
        if failed_import {
            "partial"
        } else {
            "completed"
        }
    );
    assert!(batch.source.get("archive_path").is_none());
    assert!(!fixture.path.exists());
    let job = activity_archive_import_jobs::Entity::find_by_id(fixture.job)
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (job.imported_count, job.failed_count),
        if failed_import { (16, 1) } else { (17, 0) }
    );
    let output = pipeline_outputs::Entity::find()
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output.available_at.is_some(), !failed_import);
    assert!(activity_import_locks::Entity::find_by_id(fixture.lock)
        .one(&fixture.db)
        .await
        .unwrap()
        .is_none());
}
impl Drop for ArchiveFixture {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
        std::fs::remove_dir_all(&self.uploads).ok();
    }
}

#[tokio::test]
async fn queued_archive_pages_are_atomic_replayable_and_hold_sources_until_children_finish() {
    for failed_import in [false, true] {
        let fixture = ArchiveFixture::new().await;
        fixture.page().await;
        assert_eq!(
            activity_imports::Entity::find()
                .count(&fixture.db)
                .await
                .unwrap(),
            16
        );
        batches::reconcile(&fixture.db).await.unwrap();
        assert!(fixture.path.exists());
        assert!(activity_import_locks::Entity::find_by_id(fixture.lock)
            .one(&fixture.db)
            .await
            .unwrap()
            .is_some());
        fixture.page().await;
        fixture.page().await; // Redelivery of the sealed producer has no effects.
        assert_eq!(
            activity_imports::Entity::find()
                .count(&fixture.db)
                .await
                .unwrap(),
            17
        );
        let tasks = background_tasks::Entity::find()
            .all(&fixture.db)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 19); // Parent, sixteen children, continuation, last child.
        assert_eq!(
            tasks
                .iter()
                .filter(|task| task.task_type == "activity_archive_import")
                .count(),
            2
        );
        assert_eq!(
            tasks
                .iter()
                .filter(|task| task.task_type == "process_activity_import")
                .count(),
            17
        );
        assert!(tasks
            .iter()
            .all(|task| !task.payload.to_string().contains("<gpx>")));
        fixture.finish_children(failed_import).await;
        assert_archive_barrier(&fixture, failed_import).await;
    }
}

#[tokio::test]
async fn queued_archive_download_rejection_fails_job_and_releases_its_lock() {
    let fixture = ArchiveFixture::new().await;
    worker_batches::Entity::delete_by_id(fixture.parent)
        .exec(&fixture.db)
        .await
        .unwrap();
    activity_archive_import_jobs::Entity::update_many()
        .col_expr(
            activity_archive_import_jobs::Column::ArchiveUrl,
            Expr::value("ftp://example.test/archive.zip"),
        )
        .exec(&fixture.db)
        .await
        .unwrap();
    let error = queue_archive_page(
        &fixture.db,
        &fixture.uploads,
        fixture.parent,
        fixture.job,
        None,
    )
    .await
    .unwrap_err();
    assert!(error.message.contains("https"));
    let job = activity_archive_import_jobs::Entity::find_by_id(fixture.job)
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, "failed");
    assert!(job.finished_at.is_some());
    assert!(activity_import_locks::Entity::find_by_id(fixture.lock)
        .one(&fixture.db)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        activity_imports::Entity::find()
            .count(&fixture.db)
            .await
            .unwrap(),
        0
    );
    assert!(Utc::now() >= job.finished_at.unwrap());
}
