use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_training_analyses")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub user_id: i32,
    pub ride_focus: String,
    pub route_family_key: Option<String>,
    pub comparable_distance_bucket_meters: Option<i32>,
    pub comparable_elevation_gain_bucket_meters: Option<i32>,
    pub aerobic_decoupling_percent: Option<f64>,
    pub z2_time_seconds: i32,
    pub z2_distance_meters: Option<f64>,
    pub z2_average_speed_mps: Option<f64>,
    pub climbing_time_seconds: i32,
    pub climbing_elevation_gain_meters: Option<f64>,
    pub sustained_climb_count: i32,
    pub created_at: DateTime<Utc>,
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
