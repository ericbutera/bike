use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "analytics_user_states")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    pub last_activity_change_at: DateTime<Utc>,
    pub fitness_dirty_from_day: Option<NaiveDate>,
    pub last_fitness_rebuild_at: Option<DateTime<Utc>>,
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

impl Model {
    pub async fn for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(user_id).one(db).await
    }
}

use sea_orm::QueryFilter;
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
                .col_expr(
                    Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(rebuilt_at),
                )
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
            fitness_dirty_from_day: Set(None),
            last_fitness_rebuild_at: Set(Some(rebuilt_at)),
            created_at: Set(rebuilt_at),
            updated_at: Set(rebuilt_at),
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
