use super::dataset;
use bike_core::analytics::rebuild_segment_analytics_cache;
use bike_core::auth::entities::users;
use bike_core::entities::{activities, activity_analytics, segment_efforts, segment_summaries};
use bike_core::test_support as fixtures;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, IntoActiveModel, PaginatorTrait,
    QueryFilter, Set, Statement, TransactionTrait,
};

pub async fn connect() -> DatabaseConnection {
    let project = std::env::var("BIKE_PERF_PROJECT").expect("run mise run perf:small");
    assert!(project.starts_with("bike-e2e-perf-"));
    let url = std::env::var("BIKE_PERF_DATABASE_URL").unwrap();
    let parsed = reqwest::Url::parse(&url).unwrap();
    assert_eq!(parsed.host_str(), Some(format!("{project}-postgres-1").as_str()));
    assert_eq!(parsed.path(), "/bike");
    bike_core::db::connect_database(&url).await.unwrap()
}

async fn seed_source_rows(db: &DatabaseConnection) {
    assert_eq!(
        users::Entity::find().count(db).await.unwrap(),
        0,
        "seed requires an empty disposable database"
    );
    let tx = db.begin().await.unwrap();
    for owner in 1..=dataset::OWNERS {
        seed_owner(&tx, owner).await;
    }
    fixtures::insert(&tx, dataset::session().into_active_model()).await;
    tx.commit().await.unwrap();
}

pub async fn seed(db: &DatabaseConnection) {
    seed_source_rows(db).await;
    rebuild_segment_analytics_cache(db, &(1..=dataset::OWNERS).collect::<Vec<_>>())
        .await
        .unwrap();
    assert_prepared(db).await;
    mark_missing_and_stale(db).await;
}

async fn mark_missing_and_stale(db: &DatabaseConnection) {
    for occurrence in 1..=2 {
        fixtures::update(
            db,
            segment_efforts::ActiveModel {
                id: Set(dataset::effort(1, 2, occurrence).id),
                overall_rank: Set(None),
                user_rank: Set(None),
                ..Default::default()
            },
        )
        .await;
    }
    activity_analytics::Model::delete_by_activity_ids(db, &[1002])
        .await
        .unwrap();
    mark_stale(db).await;
}

async fn mark_stale(db: &DatabaseConnection) {
    // Ordinary model updates touch updated_at; explicitly set old projection
    // timestamps through a bulk fixture patch so this really exercises stale reads.
    let stale_at = fixtures::timestamp() - chrono::Duration::days(1);
    segment_efforts::Entity::update_many()
        .set(segment_efforts::ActiveModel {
            updated_at: Set(stale_at),
            ..Default::default()
        })
        .filter(segment_efforts::Column::ActivityId.eq(1003))
        .exec(db)
        .await
        .unwrap();
    activity_analytics::Entity::update_many()
        .set(activity_analytics::ActiveModel {
            updated_at: Set(stale_at),
            ..Default::default()
        })
        .filter(activity_analytics::Column::ActivityId.eq(1003))
        .exec(db)
        .await
        .unwrap();
    let efforts = segment_efforts::Model::list_by_user_activity_ids(db, 1, &[1003])
        .await
        .unwrap();
    assert_eq!(efforts.len(), 2);
    assert!(efforts.iter().all(|effort| effort.updated_at == stale_at));
}

async fn assert_prepared(db: &DatabaseConnection) {
    assert_eq!(activities::Entity::find().count(db).await.unwrap(), 1000);
    assert_eq!(segment_efforts::Entity::find().count(db).await.unwrap(), 1980);
    let summary = segment_summaries::Entity::find_by_id(1).one(db).await.unwrap().unwrap();
    assert_eq!(summary.effort_count, 198);
    assert_eq!(summary.best_duration_seconds, Some(150));
    assert_tied_ranks(db).await;
    assert_storage_budget(db).await;
}

async fn assert_tied_ranks(db: &DatabaseConnection) {
    let fastest = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::SegmentId.eq(1))
        .filter(segment_efforts::Column::DurationSeconds.eq(150))
        .all(db)
        .await
        .unwrap();
    assert_eq!(fastest.len(), 40);
    let mut ranks: Vec<_> = fastest.iter().map(|effort| effort.overall_rank.unwrap()).collect();
    ranks.sort_unstable();
    assert_eq!(ranks, (1..=40).collect::<Vec<_>>());
    assert!(fastest.iter().all(|effort| effort.user_rank == effort.overall_rank));
}

async fn assert_storage_budget(db: &DatabaseConnection) {
    let bytes = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT pg_database_size(current_database()) AS bytes",
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(bytes.try_get::<i64>("", "bytes").unwrap() < dataset::STORAGE_BUDGET);
}

async fn seed_owner(db: &impl ConnectionTrait, owner: i32) {
    fixtures::insert(db, dataset::owner(owner).into_active_model()).await;
    fixtures::insert(db, dataset::segment(owner).into_active_model()).await;
    for index in 1..=dataset::ACTIVITIES {
        fixtures::insert(db, dataset::activity(owner, index).into_active_model()).await;
        if index == 1 {
            continue;
        }
        for occurrence in 1..=2 {
            fixtures::insert(db, dataset::effort(owner, index, occurrence).into_active_model()).await;
        }
    }
}
