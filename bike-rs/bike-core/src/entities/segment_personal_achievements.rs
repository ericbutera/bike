use sea_orm::entity::prelude::*;
use sea_orm::{IntoActiveModel, QueryOrder};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "segment_personal_achievements")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub segment_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub position: i32,
    pub effort_id: i32,
    pub duration_seconds: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Return the saved personal positions while background processing refreshes them.
    pub async fn for_user_segment(
        db: &impl ConnectionTrait,
        user_id: i32,
        segment_id: i32,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::SegmentId.eq(segment_id))
            .filter(Column::UserId.eq(user_id))
            .order_by_asc(Column::Position)
            .all(db)
            .await
    }

    pub(crate) async fn for_user_segments(
        db: &impl ConnectionTrait,
        user_id: i32,
        segment_ids: &[i32],
    ) -> Result<Vec<Self>, DbErr> {
        if segment_ids.is_empty() {
            return Ok(Vec::new());
        }
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .order_by_asc(Column::SegmentId)
            .order_by_asc(Column::Position)
            .all(db)
            .await
    }

    pub(crate) async fn replace_for_segment(
        db: &impl ConnectionTrait,
        segment_id: i32,
        podium: Vec<Self>,
    ) -> Result<(), DbErr> {
        Entity::delete_many()
            .filter(Column::SegmentId.eq(segment_id))
            .exec(db)
            .await?;
        if !podium.is_empty() {
            Entity::insert_many(podium.into_iter().map(IntoActiveModel::into_active_model))
                .exec_without_returning(db)
                .await?;
        }
        Ok(())
    }
}
