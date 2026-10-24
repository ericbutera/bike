use super::activities;
use crate::{
    activity_cards::ActivityCardPreviewPoint,
    activity_sport::ActivitySport,
    bike_tasks::{ActivityTask, Priority},
    platform::data::pagination::{PaginatedResponse, PaginationParams},
    workflow_error::WorkflowError,
};
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{
    sea_query::{Expr, OnConflict},
    FromQueryResult, JoinType, PaginatorTrait, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_cards")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub route_location: Option<String>,
    pub location: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "activities::Entity",
        from = "Column::ActivityId",
        to = "activities::Column::Id"
    )]
    Activity,
}
impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, FromQueryResult)]
pub struct CardRow {
    pub id: i32,
    pub title: String,
    pub sport: String,
    pub started_at: DateTime<Utc>,
    pub location: Option<String>,
    pub distance_meters: Option<f64>,
    pub moving_time_seconds: Option<i32>,
    pub total_time_seconds: Option<i32>,
    pub elevation_gain_meters: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<i32>,
}

#[derive(FromQueryResult)]
struct TaskRow {
    activity_id: i32,
    user_id: i32,
}

impl Model {
    pub async fn paginate_for_user(
        db: &DatabaseConnection,
        user_id: i32,
        pagination: &PaginationParams,
        sport: Option<ActivitySport>,
    ) -> Result<PaginatedResponse<CardRow>, DbErr> {
        use activities::Column as A;
        let mut query = activities::Entity::find()
            .select_only()
            .columns([
                A::Id,
                A::Title,
                A::Sport,
                A::StartedAt,
                A::DistanceMeters,
                A::MovingTimeSeconds,
                A::TotalTimeSeconds,
                A::ElevationGainMeters,
                A::AverageSpeedMps,
                A::AverageHeartRateBpm,
            ])
            .column(Column::Location)
            .join(JoinType::LeftJoin, Relation::Activity.def().rev())
            .filter(A::UserId.eq(user_id));
        if let Some(sport) = sport {
            query = query.filter(A::Sport.is_in(sport.stored_values().iter().copied()));
        }
        let (page, per_page) = pagination.normalized();
        let paginator = query
            .order_by_desc(A::StartedAt)
            .order_by_desc(A::Id)
            .into_model::<CardRow>()
            .paginate(db, per_page as u64);
        let total = paginator.num_items().await?;
        let rows = paginator.fetch_page((page - 1) as u64).await?;
        Ok(PaginatedResponse::new(rows, page, per_page, total as i64))
    }

    pub async fn backfill_page(db: &impl ConnectionTrait, after_id: i32) -> Result<Vec<ActivityTask>, DbErr> {
        use activities::Column as A;
        let rows = activities::Entity::find()
            .select_only()
            .column_as(A::Id, "activity_id")
            .column(A::UserId)
            .filter(A::Id.gt(after_id))
            .order_by_asc(A::Id)
            .limit(16)
            .into_model::<TaskRow>()
            .all(db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| ActivityTask {
                activity_id: row.activity_id,
                user_id: row.user_id,
                priority: Priority::Background,
            })
            .collect())
    }

    pub async fn set_location(
        db: &impl ConnectionTrait,
        activity_id: i32,
        location: Option<String>,
    ) -> Result<(), DbErr> {
        let value = location.map_or_else(|| Expr::col(Column::RouteLocation), Expr::value);
        Entity::update_many()
            .col_expr(Column::Location, value)
            .filter(Column::ActivityId.eq(activity_id))
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn replace_output(
        db: &DatabaseConnection,
        source: &activities::Model,
        route_location: Option<String>,
        location: Option<String>,
        points: &[ActivityCardPreviewPoint],
    ) -> Result<(), WorkflowError> {
        let txn = db.begin().await?;
        let Some(current) = activities::Model::lock_owned(&txn, source.id, source.user_id).await? else {
            return Ok(());
        };
        if current.derived_data_json != source.derived_data_json {
            return Err(WorkflowError::internal(
                "Recording or metadata changed while preparing activity card; retry",
            ));
        }
        Entity::insert(ActiveModel {
            activity_id: Set(source.id),
            route_location: Set(route_location),
            location: Set(location),
        })
        .on_conflict(
            OnConflict::column(Column::ActivityId)
                .update_columns([Column::RouteLocation, Column::Location])
                .to_owned(),
        )
        .exec_without_returning(&txn)
        .await?;
        preview_points::Entity::delete_many()
            .filter(preview_points::Column::ActivityId.eq(source.id))
            .exec(&txn)
            .await?;
        if !points.is_empty() {
            preview_points::Entity::insert_many(points.iter().enumerate().map(|(index, point)| {
                preview_points::ActiveModel {
                    activity_id: Set(source.id),
                    point_number: Set(index as i32 + 1),
                    elapsed_seconds: Set(point.elapsed_seconds),
                    latitude: Set(point.latitude),
                    longitude: Set(point.longitude),
                }
            }))
            .exec_without_returning(&txn)
            .await?;
        }
        txn.commit().await?;
        Ok(())
    }
}

pub mod preview_points {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "activity_card_preview_points")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub activity_id: i32,
        #[sea_orm(primary_key, auto_increment = false)]
        pub point_number: i32,
        pub elapsed_seconds: i32,
        pub latitude: f64,
        pub longitude: f64,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "activities::Entity",
            from = "Column::ActivityId",
            to = "activities::Column::Id"
        )]
        Activity,
    }
    impl ActiveModelBehavior for ActiveModel {}

    impl Model {
        pub async fn for_owned_activities(
            db: &impl ConnectionTrait,
            user_id: i32,
            activity_ids: &[i32],
        ) -> Result<HashMap<i32, Vec<ActivityCardPreviewPoint>>, DbErr> {
            if activity_ids.is_empty() {
                return Ok(HashMap::new());
            }
            let rows = Entity::find()
                .join(JoinType::InnerJoin, Relation::Activity.def())
                .filter(activities::Column::UserId.eq(user_id))
                .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
                .order_by_asc(Column::ActivityId)
                .order_by_asc(Column::PointNumber)
                .all(db)
                .await?;
            let mut grouped = HashMap::<_, Vec<_>>::new();
            for row in rows {
                grouped
                    .entry(row.activity_id)
                    .or_default()
                    .push(ActivityCardPreviewPoint {
                        elapsed_seconds: row.elapsed_seconds,
                        latitude: row.latitude,
                        longitude: row.longitude,
                    });
            }
            Ok(grouped)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn compact_pages_filter_the_owner_and_sport_before_loading_saved_output() {
        for user_id in [7, 8] {
            let activity = activities::Model {
                user_id,
                sport: "hike".into(),
                ..fixtures::activity()
            };
            let row = fixtures::activity_card_row(&activity, Some("Saved city".into()));
            let db = fixtures::postgres_mock()
                .append_query_results([vec![fixtures::count_row(1)]])
                .append_query_results([vec![row]])
                .into_connection();
            let result = Model::paginate_for_user(
                &db,
                user_id,
                &PaginationParams { page: 2, per_page: 1 },
                Some(ActivitySport::Hike),
            )
            .await
            .unwrap();
            assert_eq!(result.metadata.total, 1);
            assert_eq!(result.metadata.page, 2);
            assert_eq!(result.data[0].location.as_deref(), Some("Saved city"));
            let log = db.into_transaction_log();
            for transaction in &log {
                assert_eq!(
                    transaction.statements()[0].values.as_ref().unwrap().0[0],
                    user_id.into()
                );
            }
            let sql = format!("{log:?}");
            assert!(sql.contains("LEFT JOIN"));
            assert!(sql.contains("user_id"));
            assert!(sql.contains("hike"));
            assert!(sql.contains("LIMIT"));
            assert!(sql.contains("OFFSET"));
            assert!(!sql.contains("derived_data_json"));
            assert!(!sql.contains("heart_rate_zones_json"));
        }
    }

    #[tokio::test]
    async fn backfill_visits_all_sports_in_bounded_id_order_without_loading_recordings() {
        let row: BTreeMap<String, Value> = [("activity_id".into(), 43.into()), ("user_id".into(), 8.into())].into();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![row]])
            .into_connection();
        let tasks = Model::backfill_page(&db, 42).await.unwrap();
        assert_eq!(
            tasks,
            vec![ActivityTask {
                activity_id: 43,
                user_id: 8,
                priority: Priority::Background,
            }]
        );
        let log = db.into_transaction_log();
        let query = &log[0].statements()[0].sql;
        assert!(query.contains("ORDER BY"));
        assert!(query.contains("LIMIT"));
        assert!(!query.contains("sport"));
        assert!(!query.contains("derived_data_json"));
    }

    #[tokio::test]
    async fn previews_are_loaded_for_one_owners_page_in_saved_point_order() {
        for user_id in [7, 8] {
            let rows = vec![
                preview_points::Model {
                    activity_id: 42,
                    point_number: 1,
                    elapsed_seconds: 9,
                    latitude: 45.1,
                    longitude: -122.1,
                },
                preview_points::Model {
                    activity_id: 42,
                    point_number: 2,
                    elapsed_seconds: 3,
                    latitude: 45.2,
                    longitude: -122.2,
                },
                preview_points::Model {
                    activity_id: 43,
                    point_number: 1,
                    elapsed_seconds: 0,
                    latitude: 44.2,
                    longitude: -121.2,
                },
            ];
            let db = fixtures::postgres_mock().append_query_results([rows]).into_connection();
            let result = preview_points::Model::for_owned_activities(&db, user_id, &[42, 43])
                .await
                .unwrap();
            assert_eq!(
                result[&42]
                    .iter()
                    .map(|point| point.elapsed_seconds)
                    .collect::<Vec<_>>(),
                vec![9, 3]
            );
            assert_eq!(result[&43][0].latitude, 44.2);
            let log = db.into_transaction_log();
            assert_eq!(log.len(), 1);
            assert_eq!(log[0].statements()[0].values.as_ref().unwrap().0[0], user_id.into());
            let sql = &log[0].statements()[0].sql;
            assert!(sql.contains("user_id"));
            assert!(sql.contains(" IN "));
            assert!(sql.contains("point_number"));
        }
        let db = fixtures::postgres_mock().into_connection();
        assert!(preview_points::Model::for_owned_activities(&db, 7, &[])
            .await
            .unwrap()
            .is_empty());
        assert!(db.into_transaction_log().is_empty());
    }
}
