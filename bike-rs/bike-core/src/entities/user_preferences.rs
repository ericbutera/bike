use crate::training_data::StoredHeartRateZoneBounds;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "user_preferences")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    #[sea_orm(default_value = "imperial")]
    pub unit_system: String,
    pub estimated_ftp_watts: Option<i32>,
    pub heart_rate_zone_bounds_json: Option<StoredHeartRateZoneBounds>,
    pub xc_goal_start_date: Option<NaiveDate>,
    pub xc_goal_target_date: Option<NaiveDate>,
    pub xc_goal_target_distance_meters: Option<f64>,
    pub xc_goal_target_elevation_gain_meters: Option<f64>,
    pub xc_goal_event_name: Option<String>,
    pub xc_goal_target_finish_time_seconds: Option<i32>,
    pub xc_goal_event_profile: Option<String>,
    pub xc_goal_backfill_status: Option<String>,
    pub xc_goal_backfill_completed_at: Option<DateTime<Utc>>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::timestamps::impl_timestamps!(ActiveModel);

impl Model {
    pub async fn for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Option<Self>, DbErr> {
        use sea_orm::QueryFilter;
        Entity::find().filter(Column::UserId.eq(user_id)).one(db).await
    }
}

mod mutations;
pub use mutations::{GoalBackfillChange, PreferencesUpdate, SavedPreferences};
