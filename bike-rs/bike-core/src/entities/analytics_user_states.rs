use crate::entities::analytics_user_states;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::QueryFilter;
use sea_orm::{ConnectionTrait, DbErr, Set};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "analytics_user_states")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub last_activity_change_at: DateTime<Utc>,
    pub fitness_dirty_from_day: Option<NaiveDate>,
    pub last_fitness_rebuild_at: Option<DateTime<Utc>>,
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
        Entity::find_by_id(user_id).one(db).await
    }
}

impl Model {
    pub async fn complete_rebuild<C>(
        db: &C,
        user_id: i32,
        state: Option<Model>,
        rebuilt_at: DateTime<Utc>,
    ) -> Result<bool, sea_orm::DbErr>
    where
        C: ConnectionTrait,
    {
        if let Some(model) = state {
            let changed = Entity::update_many()
                .col_expr(
                    Column::FitnessDirtyFromDay,
                    sea_orm::sea_query::Expr::value(None::<NaiveDate>),
                )
                .col_expr(
                    Column::LastFitnessRebuildAt,
                    sea_orm::sea_query::Expr::value(rebuilt_at),
                )
                .col_expr(Column::UpdatedAt, sea_orm::sea_query::Expr::value(rebuilt_at))
                .filter(Column::UserId.eq(user_id))
                .filter(Column::LastActivityChangeAt.eq(model.last_activity_change_at))
                .filter(Column::UpdatedAt.eq(model.updated_at))
                .exec(db)
                .await?;
            return Ok(changed.rows_affected > 0);
        }

        let inserted = Entity::insert(ActiveModel {
            user_id: Set(user_id),
            last_activity_change_at: Set(rebuilt_at),
            last_fitness_rebuild_at: Set(Some(rebuilt_at)),
            created_at: Set(rebuilt_at),
            updated_at: Set(rebuilt_at),
            ..Default::default()
        })
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(Column::UserId)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        Ok(inserted > 0)
    }
}

impl Model {
    pub async fn mark_changed(
        db: &impl ConnectionTrait,
        user_id: i32,
        changed_at: DateTime<Utc>,
    ) -> Result<(), sea_orm::DbErr> {
        if let Some(model) = analytics_user_states::Model::for_user(db, user_id).await? {
            let mut active_model: analytics_user_states::ActiveModel = model.into();
            active_model.last_activity_change_at = Set(changed_at);
            active_model.update(db).await?;
        } else {
            analytics_user_states::ActiveModel {
                user_id: Set(user_id),
                last_activity_change_at: Set(changed_at),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }

        Ok(())
    }
    pub async fn mark_dirty(
        db: &impl ConnectionTrait,
        user_id: i32,
        dirty_from_day: NaiveDate,
        changed_at: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        use sea_orm::sea_query::{Expr, Func, OnConflict};
        // Merge against the row at write time, including concurrent first inserts.
        // A fresh write timestamp fences rebuilds even if the source time is older.
        Entity::insert(ActiveModel {
            user_id: Set(user_id),
            last_activity_change_at: Set(changed_at),
            fitness_dirty_from_day: Set(Some(dirty_from_day)),
            updated_at: Set(Utc::now()),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(Column::UserId)
                .value(
                    Column::FitnessDirtyFromDay,
                    Func::least([
                        Func::coalesce([
                            Expr::col((Entity, Column::FitnessDirtyFromDay)),
                            Expr::value(dirty_from_day),
                        ])
                        .into(),
                        Expr::value(dirty_from_day),
                    ]),
                )
                .value(
                    Column::LastActivityChangeAt,
                    Func::greatest([
                        Expr::col((Entity, Column::LastActivityChangeAt)),
                        Expr::value(changed_at),
                    ]),
                )
                .update_column(Column::UpdatedAt)
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        Ok(())
    }
}
