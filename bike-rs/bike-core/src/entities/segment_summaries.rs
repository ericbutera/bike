use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "segment_summaries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub segment_id: i32,
    pub effort_count: i32,
    pub leader_user_id: Option<i32>,
    pub leader_effort_id: Option<i32>,
    pub best_duration_seconds: Option<i32>,
    pub latest_activity_started_at: Option<DateTime<Utc>>,
    pub latest_activity_id: Option<i32>,
    pub latest_effort_id: Option<i32>,
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

impl Entity {
    pub async fn delete_for_segment(
        db: &impl ConnectionTrait,
        segment_id: i32,
    ) -> Result<(), DbErr> {
        use sea_orm::QueryFilter;
        Self::delete_many()
            .filter(Column::SegmentId.eq(segment_id))
            .exec(db)
            .await?;
        Ok(())
    }
    pub async fn for_segments(
        db: &impl ConnectionTrait,
        segment_ids: &[i32],
    ) -> Result<Vec<Model>, DbErr> {
        use sea_orm::QueryFilter;
        Self::find()
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .all(db)
            .await
    }
}

impl Entity {
    pub async fn clear_segments(
        db: &impl ConnectionTrait,
        ids: &[i32],
    ) -> Result<sea_orm::DeleteResult, DbErr> {
        use sea_orm::QueryFilter;
        Self::delete_many()
            .filter(Column::SegmentId.is_in(ids.iter().copied()))
            .exec(db)
            .await
    }
}
