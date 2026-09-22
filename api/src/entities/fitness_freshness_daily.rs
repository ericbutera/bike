use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, QueryOrder, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "fitness_freshness_daily")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub day: NaiveDate,
    pub activity_count: i32,
    pub training_load: f64,
    pub fitness: f64,
    pub fatigue: f64,
    pub form: f64,
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
    pub async fn latest_before_day<C>(
        db: &C,
        user_id: i32,
        day: NaiveDate,
    ) -> Result<Option<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Day.lt(day))
            .order_by_desc(Column::Day)
            .one(db)
            .await
    }

    pub async fn replace_user_rows_from_day<C>(
        db: &C,
        user_id: i32,
        rebuild_from_day: Option<NaiveDate>,
        rows: impl IntoIterator<Item = ActiveModel>,
    ) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        let mut delete_query = Entity::delete_many().filter(Column::UserId.eq(user_id));

        if let Some(rebuild_from_day) = rebuild_from_day {
            delete_query = delete_query.filter(Column::Day.gte(rebuild_from_day));
        }

        delete_query.exec(db).await?;

        let rows = rows.into_iter().collect::<Vec<_>>();
        for chunk in rows.chunks(200) {
            Entity::insert_many(chunk.iter().cloned()).exec(db).await?;
        }

        Ok(())
    }
}
