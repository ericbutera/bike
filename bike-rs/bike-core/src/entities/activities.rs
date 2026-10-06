use crate::activity_data::StoredActivityDerivedData;
use crate::activity_sport::is_bike_activity_sport;
pub use crate::activity_sport::BIKE_ACTIVITY_SPORT_VALUES;
use crate::training_data::StoredActivityHeartRateZones;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Set,
};
use std::collections::HashMap;

pub const BIKE_ACTIVITY_SPORT: &str = "ride";

// PostgreSQL maintains this summary from the full recording evidence. It is
// intentionally absent from ActiveModel: application writes cannot override it.
#[derive(sea_orm::DeriveIden)]
enum RecordingColumn {
    RecordingEnvironment,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub activity_import_id: Option<i32>,
    pub title: String,
    pub sport: String,
    pub source: String,
    pub source_correlation_id: Option<String>,
    pub original_filename: Option<String>,
    pub format: Option<String>,
    pub activity_type: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub distance_meters: Option<f64>,
    pub moving_time_seconds: Option<i32>,
    pub total_time_seconds: Option<i32>,
    pub elevation_gain_meters: Option<f64>,
    pub elevation_loss_meters: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub max_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<i32>,
    pub max_cadence_rpm: Option<i32>,
    pub calories: Option<i32>,
    pub estimated_ftp_watts: Option<i32>,
    pub heart_rate_zones_json: Option<StoredActivityHeartRateZones>,
    pub derived_data_json: Option<StoredActivityDerivedData>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let now = Utc::now();
        if insert {
            self.created_at = Set(now);
        }
        self.updated_at = Set(now);
        Ok(self)
    }
}

#[derive(Clone, Debug, FromQueryResult)]
pub struct ActivityStartedAtRow {
    pub id: i32,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, FromQueryResult)]
pub struct ActivityMatchingRoute {
    pub id: i32,
    pub user_id: i32,
    pub derived_data_json: Option<StoredActivityDerivedData>,
}

#[derive(Clone, Debug, FromQueryResult)]
pub struct ActivityTrainingLoadRow {
    pub started_at: DateTime<Utc>,
    pub moving_time_seconds: Option<i32>,
    pub total_time_seconds: Option<i32>,
    pub average_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub heart_rate_zones_json: Option<StoredActivityHeartRateZones>,
}

impl Model {
    pub async fn recording_page(
        db: &impl ConnectionTrait,
        user_id: i32,
        after_id: i32,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.gt(after_id))
            .filter(Column::Sport.is_in(BIKE_ACTIVITY_SPORT_VALUES.iter().copied()))
            .order_by_asc(Column::Id)
            .limit(limit.clamp(1, 32))
            .all(db)
            .await
    }
    pub async fn withhold_unavailable_source(
        &self,
        db: &impl ConnectionTrait,
    ) -> Result<bool, DbErr> {
        Ok(Entity::update_many()
            .set(ActiveModel {
                format: Set(Some("unavailable".into())),
                original_filename: Set(None),
                updated_at: Set(Utc::now()),
                ..Default::default()
            })
            .filter(Column::Id.eq(self.id))
            .filter(Column::UserId.eq(self.user_id))
            .filter(Column::UpdatedAt.eq(self.updated_at))
            .exec(db)
            .await?
            .rows_affected
            == 1)
    }
    pub fn recording_context(&self) -> crate::activity_recording::RecordingContext {
        let mut context = crate::activity_data::deserialize_derived_activity_data(
            self.derived_data_json.as_ref(),
        )
        .recording;
        context.merge(crate::activity_recording::RecordingContext::legacy(
            &self.sport,
            &self.source,
            &self.title,
        ));
        context
    }

    pub async fn recording_counterparts(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_id: i32,
        started_at: DateTime<Utc>,
    ) -> Result<Vec<RecordingCandidate>, DbErr> {
        Entity::find()
            .select_only()
            .columns([
                Column::Id,
                Column::StartedAt,
                Column::UpdatedAt,
                Column::Sport,
                Column::Source,
                Column::Title,
                Column::DerivedDataJson,
            ])
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.ne(activity_id))
            .filter(Column::StartedAt.between(
                started_at - chrono::Duration::seconds(5),
                started_at + chrono::Duration::seconds(5),
            ))
            .order_by_asc(Column::Id)
            .limit(32)
            .into_model::<RecordingCandidate>()
            .all(db)
            .await
    }

    pub async fn store_recording(
        db: &impl ConnectionTrait,
        activity_id: i32,
        user_id: i32,
        updated_at: DateTime<Utc>,
        derived: &crate::activity_data::ActivityDerivedData,
    ) -> Result<bool, DbErr> {
        Ok(Entity::update_many()
            .set(ActiveModel {
                derived_data_json: Set(Some(
                    crate::activity_data::serialize_derived_activity_data(derived),
                )),
                updated_at: Set(Utc::now()),
                ..Default::default()
            })
            .filter(Column::Id.eq(activity_id))
            .filter(Column::UserId.eq(user_id))
            .filter(Column::UpdatedAt.eq(updated_at))
            .exec(db)
            .await?
            .rows_affected
            == 1)
    }

    pub async fn find_owned(
        db: &impl ConnectionTrait,
        activity_id: i32,
        user_id: i32,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(activity_id)
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await
    }

    pub async fn lock_owned(
        db: &impl ConnectionTrait,
        activity_id: i32,
        user_id: i32,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(activity_id)
            .filter(Column::UserId.eq(user_id))
            .lock_exclusive()
            .one(db)
            .await
    }

    pub fn heatmap_eligible() -> sea_orm::Condition {
        use sea_orm::{
            sea_query::{extension::postgres::PgExpr, Expr, Func},
            ExprTrait,
        };
        let environment = Expr::col((Entity, RecordingColumn::RecordingEnvironment));
        sea_orm::Condition::all()
            .add(Column::Sport.is_in(BIKE_ACTIVITY_SPORT_VALUES.iter().copied()))
            .add(Func::coalesce([Column::Format.into_expr(), Expr::val("")]).ne("unavailable"))
            .add(
                Func::coalesce([environment, Expr::val("unknown")])
                    .is_not_in(["indoor", "virtual"]),
            )
            .add(Self::eligible_recording_token(Column::Sport))
            .add(Self::eligible_recording_token(Column::Source))
            .add(Column::Title.into_expr().not_ilike("%zwift%"))
    }

    fn eligible_recording_token(column: Column) -> sea_orm::Condition {
        use sea_orm::{
            sea_query::{extension::postgres::PgExpr, Alias, Expr, Func},
            ExprTrait,
        };
        let token = Func::cust(Alias::new("regexp_replace")).args([
            Func::lower(column.into_expr()).into(),
            Expr::val("[^a-z0-9]"),
            Expr::val(""),
            Expr::val("g"),
        ]);
        sea_orm::Condition::all()
            .add(
                token.clone().is_not_in(
                    crate::activity_recording::INDOOR_RECORDING_TOKENS
                        .iter()
                        .copied(),
                ),
            )
            .add(token.clone().not_ilike("%virtual%"))
            .add(token.not_ilike("%zwift%"))
    }
    pub async fn bike_ids_for_user<C>(db: &C, user_id: i32) -> Result<Vec<i32>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Sport.is_in(BIKE_ACTIVITY_SPORT_VALUES.iter().copied()))
            .order_by_asc(Column::Id)
            .into_tuple::<i32>()
            .all(db)
            .await
    }

    pub async fn bike_for_training_analysis<C>(
        db: &C,
        activity_id: i32,
    ) -> Result<Option<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find_by_id(activity_id)
            .filter(Column::Sport.is_in(BIKE_ACTIVITY_SPORT_VALUES.iter().copied()))
            .one(db)
            .await
    }

    pub async fn matching_routes_page<C>(
        db: &C,
        user_id: i32,
        after_id: i32,
        limit: u64,
    ) -> Result<Vec<ActivityMatchingRoute>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::UserId)
            .column(Column::DerivedDataJson)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Sport.is_in(BIKE_ACTIVITY_SPORT_VALUES.iter().copied()))
            .filter(Column::Id.gt(after_id))
            .order_by_asc(Column::Id)
            .limit(limit)
            .into_model::<ActivityMatchingRoute>()
            .all(db)
            .await
    }

    pub async fn list_training_loads_for_fitness_rebuild<C>(
        db: &C,
        user_id: i32,
        rebuild_from_day: Option<NaiveDate>,
    ) -> Result<Vec<ActivityTrainingLoadRow>, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut query = Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Sport.is_in(BIKE_ACTIVITY_SPORT_VALUES.iter().copied()))
            .order_by_asc(Column::StartedAt);

        if let Some(rebuild_from_day) = rebuild_from_day {
            let start_bound = DateTime::<Utc>::from_naive_utc_and_offset(
                rebuild_from_day
                    .and_hms_opt(0, 0, 0)
                    .expect("valid start of day"),
                Utc,
            );
            query = query.filter(Column::StartedAt.gte(start_bound));
        }

        query
            .select_only()
            .column(Column::StartedAt)
            .column(Column::MovingTimeSeconds)
            .column(Column::TotalTimeSeconds)
            .column(Column::AverageHeartRateBpm)
            .column(Column::MaxHeartRateBpm)
            .column(Column::HeartRateZonesJson)
            .into_model::<ActivityTrainingLoadRow>()
            .all(db)
            .await
    }

    pub async fn started_at_by_ids<C>(
        db: &C,
        activity_ids: &[i32],
    ) -> Result<HashMap<i32, DateTime<Utc>>, DbErr>
    where
        C: ConnectionTrait,
    {
        if activity_ids.is_empty() {
            return Ok(HashMap::new());
        }

        Ok(Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::StartedAt)
            .filter(Column::Id.is_in(activity_ids.iter().copied()))
            .into_model::<ActivityStartedAtRow>()
            .all(db)
            .await?
            .into_iter()
            .map(|activity| (activity.id, activity.started_at))
            .collect())
    }

    pub fn is_bike_activity(&self) -> bool {
        is_bike_activity_sport(&self.sport)
    }
}

#[derive(Debug, FromQueryResult)]
pub struct RecordingCandidate {
    pub id: i32,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub sport: String,
    pub source: String,
    pub title: String,
    pub derived_data_json: Option<StoredActivityDerivedData>,
}
