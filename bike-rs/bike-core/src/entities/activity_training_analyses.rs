use crate::activity_training_analysis::ActivityTrainingAnalysisResponse;
use crate::entities::activities;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_training_analyses")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub user_id: i32,
    #[sea_orm(default_value = "other")]
    pub ride_focus: String,
    pub route_family_key: Option<String>,
    pub comparable_distance_bucket_meters: Option<i32>,
    pub comparable_elevation_gain_bucket_meters: Option<i32>,
    pub aerobic_decoupling_percent: Option<f64>,
    #[sea_orm(default_value = 0)]
    pub z2_time_seconds: i32,
    pub z2_distance_meters: Option<f64>,
    pub z2_average_speed_mps: Option<f64>,
    #[sea_orm(default_value = 0)]
    pub climbing_time_seconds: i32,
    pub climbing_elevation_gain_meters: Option<f64>,
    #[sea_orm(default_value = 0)]
    pub sustained_climb_count: i32,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Entity {
    pub async fn delete_by_activity_id(
        db: &impl ConnectionTrait,
        activity_id: i32,
    ) -> Result<(), DbErr> {
        Self::delete_by_id(activity_id).exec(db).await?;
        Ok(())
    }
}

impl Model {
    pub async fn clear_for_non_cycling(
        db: &impl ConnectionTrait,
        activity: &super::activities::Model,
    ) -> Result<(), DbErr> {
        if !activity.is_bike_activity() {
            Entity::delete_by_activity_id(db, activity.id).await?;
        }
        Ok(())
    }
}

crate::timestamps::impl_timestamps!(ActiveModel);

impl Model {
    pub async fn for_activity(
        db: &impl ConnectionTrait,
        activity_id: i32,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(activity_id).one(db).await
    }
    pub async fn for_activity_ids(
        db: &impl ConnectionTrait,
        activity_ids: &[i32],
    ) -> Result<Vec<Self>, DbErr> {
        use sea_orm::QueryFilter;
        Entity::find()
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .all(db)
            .await
    }
    pub async fn for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Vec<Self>, DbErr> {
        use sea_orm::QueryFilter;
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .all(db)
            .await
    }
    pub async fn for_owned_activity_ids(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_ids: &[i32],
    ) -> Result<Vec<Self>, DbErr> {
        use sea_orm::QueryFilter;
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .all(db)
            .await
    }
}

impl Model {
    pub async fn clear_activity(db: &impl ConnectionTrait, id: i32) -> Result<(), DbErr> {
        Entity::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}

impl Model {
    pub async fn save_analysis(
        db: &impl ConnectionTrait,
        activity: &activities::Model,
        analysis: &ActivityTrainingAnalysisResponse,
        existing: Option<Self>,
    ) -> Result<Self, DbErr> {
        let update = existing.is_some();
        let mut active_model = existing.map(ActiveModel::from).unwrap_or_default();
        active_model.activity_id = Set(activity.id);
        active_model.user_id = Set(activity.user_id);
        active_model.ride_focus = Set(analysis.ride_focus.as_str().to_string());
        active_model.route_family_key = Set(analysis.route_family_key.clone());
        active_model.comparable_distance_bucket_meters =
            Set(analysis.comparable_distance_bucket_meters);
        active_model.comparable_elevation_gain_bucket_meters =
            Set(analysis.comparable_elevation_gain_bucket_meters);
        active_model.aerobic_decoupling_percent = Set(analysis.aerobic_decoupling_percent);
        active_model.z2_time_seconds = Set(analysis.z2_time_seconds);
        active_model.z2_distance_meters = Set(analysis.z2_distance_meters);
        active_model.z2_average_speed_mps = Set(analysis.z2_average_speed_mps);
        active_model.climbing_time_seconds = Set(analysis.climbing_time_seconds);
        active_model.climbing_elevation_gain_meters = Set(analysis.climbing_elevation_gain_meters);
        active_model.sustained_climb_count = Set(analysis.sustained_climb_count);

        if update {
            active_model.update(db).await
        } else {
            active_model.insert(db).await
        }
    }
}
