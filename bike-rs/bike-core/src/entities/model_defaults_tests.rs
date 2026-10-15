//! Representative persistence checks; each entity owns its defaults and fixtures.
use super::{activity_imports, user_preferences};
use crate::test_support::{
    insert, new_background_task, new_customer, new_feature_flag, new_user_preferences,
};
use crate::{
    auth::entities::users, background_jobs::background_tasks,
    platform::feature_flags::entities as feature_flags, test_support,
};
use chrono::{Duration, Utc};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, EntityTrait, NotSet, Schema, Set};

async fn database<E: EntityTrait>(entity: E) -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    db.execute(&schema.create_table_from_entity(entity))
        .await
        .unwrap();
    db
}

#[tokio::test]
async fn omitted_insert_fields_use_database_defaults() {
    let db = database(user_preferences::Entity).await;
    let before = Utc::now() - Duration::seconds(1);
    let model = insert(
        &db,
        user_preferences::ActiveModel {
            created_at: NotSet,
            updated_at: NotSet,
            ..new_user_preferences()
        },
    )
    .await;
    assert_eq!(model.unit_system, "imperial");
    assert_eq!(model.created_at, model.updated_at);
    assert!(model.created_at >= before && model.created_at <= Utc::now());
}

#[tokio::test]
async fn explicit_insert_values_survive_and_updates_refresh_only_updated_at() {
    let db = database(user_preferences::Entity).await;
    let historical = test_support::timestamp() - Duration::days(1);
    let model = insert(
        &db,
        user_preferences::ActiveModel {
            unit_system: Set("metric".into()),
            created_at: Set(historical),
            updated_at: Set(historical),
            ..new_user_preferences()
        },
    )
    .await;
    assert_eq!(model.unit_system, "metric");
    assert_eq!(model.created_at, historical);
    assert_eq!(model.updated_at, historical);

    let mut active: user_preferences::ActiveModel = model.into();
    active.unit_system = Set("imperial".into());
    let updated = test_support::update(&db, active).await;
    assert_eq!(updated.created_at, historical);
    assert!(updated.updated_at > historical);
    assert_eq!(updated.unit_system, "imperial");
}

#[tokio::test]
async fn account_updates_refresh_updated_at() {
    let db = database(users::Entity).await;
    let user = insert(&db, new_customer()).await;
    let updated = user
        .clone()
        .update_admin(&db, Some("Updated name".into()), None)
        .await
        .unwrap();
    assert_eq!(updated.name, "Updated name");
    assert_eq!(updated.created_at, user.created_at);
    assert!(updated.updated_at > user.updated_at);
}

#[tokio::test]
async fn flag_updates_use_the_shared_timestamp_hook() {
    let db = database(feature_flags::Entity).await;
    let historical = test_support::timestamp();
    let flag = insert(&db, new_feature_flag()).await;
    assert!(!flag.enabled);
    let updated = flag.set_enabled(&db, true).await.unwrap();
    assert!(updated.enabled);
    assert_eq!(updated.created_at, historical);
    assert!(updated.updated_at > historical);
}

#[tokio::test]
async fn task_overrides_survive_and_bulk_inserts_use_database_defaults() {
    let db = database(background_tasks::Entity).await;
    let historical = test_support::timestamp();
    let model = insert(
        &db,
        background_tasks::ActiveModel {
            status: Set("failed".into()),
            attempts: Set(2),
            max_attempts: Set(5),
            ..new_background_task()
        },
    )
    .await;
    assert_eq!(
        (model.status.as_str(), model.attempts, model.max_attempts),
        ("failed", 2, 5)
    );
    assert_eq!(model.created_at, historical);
    assert_eq!(model.updated_at, historical);

    background_tasks::Entity::insert_many(["one", "two"].map(|task_type| {
        background_tasks::ActiveModel {
            task_type: Set(task_type.into()),
            created_at: NotSet,
            updated_at: NotSet,
            ..new_background_task()
        }
    }))
    .exec(&db)
    .await
    .unwrap();
    let tasks = background_tasks::Entity::find().all(&db).await.unwrap();
    assert_eq!(tasks.len(), 3);
    for task in tasks.into_iter().filter(|task| task.id != model.id) {
        assert_eq!(
            (task.status.as_str(), task.attempts, task.max_attempts),
            ("pending", 0, 3)
        );
        assert_eq!(task.created_at, task.updated_at);
        assert!(task.created_at > historical);
    }
}

#[tokio::test]
async fn legacy_import_version_survives_insert_and_partial_updates() {
    let db = database(activity_imports::Entity).await;
    let current = insert(
        &db,
        activity_imports::ActiveModel {
            import_version: NotSet,
            ..test_support::new_activity_import()
        },
    )
    .await;
    assert_eq!(
        current.import_version,
        activity_imports::ACTIVITY_IMPORT_VERSION_CURRENT
    );

    let model = insert(
        &db,
        activity_imports::ActiveModel {
            import_version: Set(activity_imports::ACTIVITY_IMPORT_VERSION_LEGACY),
            ..test_support::new_activity_import()
        },
    )
    .await;
    assert_eq!(
        model.import_version,
        activity_imports::ACTIVITY_IMPORT_VERSION_LEGACY
    );

    let updated = test_support::update(
        &db,
        activity_imports::ActiveModel {
            id: Set(model.id),
            status: Set("failed".into()),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(updated.import_version, model.import_version);
    assert_eq!(updated.created_at, model.created_at);
    assert_eq!(updated.status, "failed");
}
