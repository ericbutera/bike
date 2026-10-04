use super::{activities, segment_efforts, segments};
use crate::auth::entities::users;
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "synthetic_scenarios")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub name: String,
    pub user_id: i32,
    pub activity_id: i32,
    pub segment_id: i32,
    pub first_effort_id: i32,
    pub second_effort_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn validate_owner<C: ConnectionTrait>(&self, db: &C) -> Result<users::Model, DbErr> {
        let invalid =
            || DbErr::Custom("Synthetic scenario ownership or account isolation is invalid".into());
        let user = users::Entity::find_by_id(self.user_id)
            .one(db)
            .await?
            .ok_or_else(invalid)?;
        if !user.is_synthetic() || !user.disabled || user.is_admin.unwrap_or(false) {
            return Err(invalid());
        }
        let activity = activities::Entity::find_by_id(self.activity_id)
            .one(db)
            .await?
            .ok_or_else(invalid)?;
        let segment = segments::Entity::find_by_id(self.segment_id)
            .one(db)
            .await?
            .ok_or_else(invalid)?;
        if activity.user_id != user.id
            || segment.user_id != user.id
            || self.first_effort_id == self.second_effort_id
        {
            return Err(invalid());
        }
        for effort_id in [self.first_effort_id, self.second_effort_id] {
            let effort = segment_efforts::Entity::find_by_id(effort_id)
                .one(db)
                .await?
                .ok_or_else(invalid)?;
            let ride = activities::Entity::find_by_id(effort.activity_id)
                .one(db)
                .await?
                .ok_or_else(invalid)?;
            if effort.user_id != user.id
                || effort.segment_id != segment.id
                || ride.user_id != user.id
            {
                return Err(invalid());
            }
        }
        Ok(user)
    }

    pub async fn find<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(name).one(db).await
    }
}
