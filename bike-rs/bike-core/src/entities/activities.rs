use crate::activity_data::StoredActivityDerivedData;
use crate::activity_sport::is_bike_activity_sport;
use crate::activity_sport::ActivitySport;
pub use crate::activity_sport::BIKE_ACTIVITY_SPORT_VALUES;
use crate::activity_type::ActivityType;
use crate::platform::data::pagination::{Paginatable, PaginatedResponse, PaginationParams};
use crate::training_data::StoredActivityHeartRateZones;
use crate::workflow_error::WorkflowError;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use std::collections::HashMap;
mod history;
pub(crate) mod parsed;
mod reprocessing;

pub const BIKE_ACTIVITY_SPORT: &str = "ride";

#[derive(Debug, Default, serde::Deserialize, utoipa::IntoParams)]
pub struct AdminListFilters {
    /// Activity title search or exact activity ID.
    pub q: Option<String>,
    /// Positive owner ID.
    pub user_id: Option<i32>,
    /// Exact import source.
    pub source: Option<String>,
    /// Normalized sport, including legacy stored spellings.
    pub sport: Option<String>,
}

#[derive(Debug, FromQueryResult)]
pub struct AdminListRow {
    pub id: i32,
    pub user_id: i32,
    pub title: String,
    pub sport: String,
    pub source: String,
    pub started_at: DateTime<Utc>,
    pub format: Option<String>,
    pub activity_import_id: Option<i32>,
}

impl Model {
    pub async fn find_by_id(
        db: &impl ConnectionTrait,
        activity_id: i32,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(activity_id).one(db).await
    }

    pub async fn effort_sources(
        db: &impl ConnectionTrait,
        activity_ids: &[i32],
    ) -> Result<Vec<EffortActivityRow>, DbErr> {
        Entity::find()
            .select_only()
            .columns([
                Column::Id,
                Column::Title,
                Column::StartedAt,
                Column::DerivedDataJson,
            ])
            .filter(Column::Id.is_in(activity_ids.iter().copied()))
            .into_model::<EffortActivityRow>()
            .all(db)
            .await
    }

    pub async fn effort_summaries(
        db: &impl ConnectionTrait,
        activity_ids: &[i32],
    ) -> Result<Vec<EffortActivitySummaryRow>, DbErr> {
        Entity::find()
            .select_only()
            .columns([Column::Id, Column::Title, Column::StartedAt])
            .filter(Column::Id.is_in(activity_ids.iter().copied()))
            .into_model::<EffortActivitySummaryRow>()
            .all(db)
            .await
    }

    pub async fn paginate_for_user(
        db: &DatabaseConnection,
        user_id: i32,
        pagination: &PaginationParams,
        sport: Option<ActivitySport>,
    ) -> Result<PaginatedResponse<Self>, DbErr> {
        let mut query = Entity::find().filter(Column::UserId.eq(user_id));
        if let Some(sport) = sport {
            query = query.filter(Column::Sport.is_in(sport.stored_values().iter().copied()));
        }
        query
            .order_by_desc(Column::StartedAt)
            .order_by_desc(Column::Id)
            .fetch_paginated(db, pagination)
            .await
    }

    /// Resolve import links within one owner, preferring the import's explicit
    /// activity link over an activity's reverse link to the import.
    pub async fn linked_to_imports(
        db: &impl ConnectionTrait,
        user_id: i32,
        imports: &[super::activity_imports::Model],
    ) -> Result<HashMap<i32, Self>, DbErr> {
        if imports.is_empty() {
            return Ok(HashMap::new());
        }
        let import_ids = imports.iter().map(|import| import.id);
        let mut linked = Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityImportId.is_in(import_ids))
            .all(db)
            .await?
            .into_iter()
            .filter_map(|activity| activity.activity_import_id.map(|id| (id, activity)))
            .collect::<HashMap<_, _>>();
        let imports_by_activity_id = imports
            .iter()
            .filter_map(|import| import.activity_id.map(|id| (id, import.id)))
            .collect::<HashMap<_, _>>();
        if !imports_by_activity_id.is_empty() {
            for activity in
                Self::list_owned_by_ids(db, user_id, imports_by_activity_id.keys().copied()).await?
            {
                if let Some(import_id) = imports_by_activity_id.get(&activity.id) {
                    linked.insert(*import_id, activity);
                }
            }
        }
        Ok(linked)
    }

    pub async fn list_owned_by_ids(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_ids: impl IntoIterator<Item = i32>,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.is_in(activity_ids))
            .all(db)
            .await
    }

    fn normalize_title(raw_title: &str) -> Result<String, WorkflowError> {
        let title = raw_title.trim();
        if title.is_empty() {
            return Err(WorkflowError::validation_field(
                "title",
                "Activity title is required",
            ));
        }
        Ok(title.to_string())
    }

    pub async fn native_strava_ids(
        db: &impl ConnectionTrait,
        user_id: i32,
    ) -> Result<Vec<i64>, DbErr> {
        let values = Entity::find()
            .select_only()
            .column(Column::SourceCorrelationId)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Source.eq("strava_sync"))
            .order_by_asc(Column::Id)
            .limit(50)
            .into_tuple::<Option<String>>()
            .all(db)
            .await?;
        values
            .into_iter()
            .flatten()
            .map(|value| {
                value
                    .parse::<i64>()
                    .map_err(|error| DbErr::Custom(error.to_string()))
            })
            .collect()
    }
    pub async fn admin_page(
        db: &impl ConnectionTrait,
        filters: &AdminListFilters,
        page: u64,
        per_page: u64,
    ) -> Result<(Vec<AdminListRow>, u64), DbErr> {
        let query = Self::admin_query(filters);
        let total = query.clone().count(db).await?;
        let rows = query
            .select_only()
            .columns([
                Column::Id,
                Column::UserId,
                Column::Title,
                Column::Sport,
                Column::Source,
                Column::StartedAt,
                Column::Format,
                Column::ActivityImportId,
            ])
            .order_by_desc(Column::StartedAt)
            .order_by_desc(Column::Id)
            .into_model::<AdminListRow>()
            .paginate(db, per_page.clamp(1, 100))
            .fetch_page(page.max(1) - 1)
            .await?;
        Ok((rows, total))
    }

    fn admin_query(filters: &AdminListFilters) -> sea_orm::Select<Entity> {
        use sea_orm::sea_query::{Expr, Func};
        use sea_orm::{Condition, ExprTrait};
        let mut query = Entity::find();
        if let Some(term) = filters
            .q
            .as_deref()
            .map(str::trim)
            .filter(|term| !term.is_empty())
        {
            query = query.filter(
                Condition::any()
                    .add(
                        Expr::expr(Func::lower(Column::Title.into_expr()))
                            .like(format!("%{}%", term.to_lowercase())),
                    )
                    .add_option(term.parse::<i32>().ok().map(|id| Column::Id.eq(id))),
            );
        }
        query.filter(
            Condition::all()
                .add_option(filters.user_id.map(|id| Column::UserId.eq(id)))
                .add_option(
                    filters
                        .source
                        .as_deref()
                        .filter(|value| !value.is_empty())
                        .map(|source| Column::Source.eq(source)),
                )
                .add_option(
                    filters
                        .sport
                        .as_deref()
                        .filter(|value| !value.is_empty())
                        .map(|sport| {
                            crate::activity_sport::activity_sport_from_stored(sport).map_or_else(
                                || Column::Sport.eq(sport),
                                |sport| Column::Sport.is_in(sport.stored_values().iter().copied()),
                            )
                        }),
                ),
        )
    }
}

impl Model {
    pub async fn metrics_counts(
        db: &DatabaseConnection,
        now: DateTime<Utc>,
    ) -> Result<crate::platform::aggregator::StoredRecordCounts, DbErr> {
        crate::platform::aggregator::Aggregator::stored_records::<Entity>(
            db,
            Column::CreatedAt,
            now,
        )
        .await
    }

    pub async fn owned_strava_activities(
        db: &impl ConnectionTrait,
        user_id: i32,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Source.eq("strava_sync"))
            .all(db)
            .await
    }
    pub async fn strava_activity(
        db: &impl ConnectionTrait,
        user_id: i32,
        vendor_id: i64,
    ) -> Result<Option<Self>, DbErr> {
        Self::strava_identity(user_id, vendor_id).one(db).await
    }

    /// Lock the current row before a provider mutation or history snapshot.
    pub async fn lock_strava_activity(
        db: &impl ConnectionTrait,
        user_id: i32,
        vendor_id: i64,
    ) -> Result<Option<Self>, DbErr> {
        Self::strava_identity(user_id, vendor_id)
            .lock_exclusive()
            .one(db)
            .await
    }

    fn strava_identity(user_id: i32, vendor_id: i64) -> sea_orm::Select<Entity> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Source.eq("strava_sync"))
            .filter(Column::SourceCorrelationId.eq(vendor_id.to_string()))
    }
    pub async fn cycling_ids_page(
        db: &impl ConnectionTrait,
        user_id: i32,
        after: i32,
    ) -> Result<Vec<i32>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Sport.is_in(BIKE_ACTIVITY_SPORT_VALUES.iter().copied()))
            .filter(Column::Id.gt(after))
            .order_by_asc(Column::Id)
            .limit(16)
            .into_tuple::<i32>()
            .all(db)
            .await
    }
    pub async fn imported_ids_page(
        db: &impl ConnectionTrait,
        user_id: i32,
        after: i32,
        archive_only: bool,
    ) -> Result<Vec<i32>, DbErr> {
        use sea_orm::QueryTrait;
        let imports = super::activity_imports::Entity::find()
            .select_only()
            .column(super::activity_imports::Column::Id)
            .filter(super::activity_imports::Column::UserId.eq(user_id))
            .filter(super::activity_imports::Column::Source.ne("strava_sync"));
        let imports = if archive_only {
            imports
                .filter(super::activity_imports::Column::Source.eq("archive_url_import"))
                .filter(super::activity_imports::Column::Format.eq("fit"))
        } else {
            imports
        };
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.gt(after))
            .filter(Column::Source.ne("strava_sync"))
            .filter(Column::ActivityImportId.in_subquery(imports.into_query()))
            .order_by_asc(Column::Id)
            .limit(16)
            .into_tuple::<i32>()
            .all(db)
            .await
    }
}

// PostgreSQL maintains this summary from the full recording evidence. It is
// intentionally absent from ActiveModel: application writes cannot override it.
#[derive(sea_orm::DeriveIden)]
enum RecordingColumn {
    RecordingEnvironment,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[sea_orm(default_value = "training")]
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
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Entity {
    pub async fn delete_by_user(
        db: &impl ConnectionTrait,
        activity_id: i32,
        user_id: i32,
    ) -> Result<(), DbErr> {
        Self::delete_by_id(activity_id)
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn count_by_import_for_user(
        db: &impl ConnectionTrait,
        activity_import_id: i32,
        user_id: i32,
    ) -> Result<u64, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityImportId.eq(activity_import_id))
            .count(db)
            .await
    }
}

crate::timestamps::impl_timestamps!(ActiveModel);

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

    /// Serialize shared projections even when their output row does not exist yet.
    /// Lock the authoritative activities in a consistent order before reading inputs.
    pub(crate) async fn lock_for_analytics(
        db: &impl ConnectionTrait,
        activity_ids: &[i32],
    ) -> Result<(), DbErr> {
        if activity_ids.is_empty() {
            return Ok(());
        }
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::Id.is_in(activity_ids.iter().copied()))
            .order_by_asc(Column::Id)
            .lock_exclusive()
            .into_tuple::<i32>()
            .all(db)
            .await?;
        Ok(())
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

#[derive(Clone, Debug, sea_orm::FromQueryResult)]
pub struct EffortActivityRow {
    pub id: i32,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub derived_data_json: Option<crate::activity_details::StoredActivityDerivedData>,
}
#[derive(Clone, Debug, sea_orm::FromQueryResult)]
pub struct EffortActivitySummaryRow {
    pub id: i32,
    pub title: String,
    pub started_at: DateTime<Utc>,
}

mod reports;

mod training;
pub use training::ActivitySummaryRow;

impl Model {
    pub async fn for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .all(db)
            .await
    }
    pub async fn owned_import_activity(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_id: i32,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityImportId.eq(Some(import_id)))
            .one(db)
            .await
    }
    pub async fn linked_by_import_ids(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_ids: &[i32],
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityImportId.is_in(import_ids.iter().copied()))
            .all(db)
            .await
    }
    pub async fn at_start_for_sports(
        db: &impl ConnectionTrait,
        user_id: i32,
        started_at: DateTime<Utc>,
        sports: &[&str],
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::StartedAt.eq(started_at))
            .filter(Column::Sport.is_in(sports.iter().copied()))
            .all(db)
            .await
    }
    pub async fn from_source_identity(
        db: &impl ConnectionTrait,
        user_id: i32,
        source: &str,
        correlation_id: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Source.eq(source))
            .filter(Column::SourceCorrelationId.eq(correlation_id))
            .one(db)
            .await
    }
    pub async fn owner_ids(db: &impl ConnectionTrait) -> Result<Vec<i32>, DbErr> {
        let mut ids = Entity::find()
            .select_only()
            .column(Column::UserId)
            .filter(Column::UserId.is_not_null())
            .distinct()
            .into_tuple::<i32>()
            .all(db)
            .await?;
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }
    pub async fn for_import_inspection(
        db: &impl ConnectionTrait,
        import: &super::activity_imports::Model,
    ) -> Result<Option<Self>, DbErr> {
        if let Some(id) = import.activity_id {
            return Self::find_by_id(db, id).await;
        }
        Entity::find()
            .filter(Column::ActivityImportId.eq(Some(import.id)))
            .one(db)
            .await
    }
}
