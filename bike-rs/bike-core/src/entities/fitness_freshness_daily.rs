use crate::analytics::FitnessFreshnessDay;
use crate::entities::fitness_freshness_daily;
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

impl Model {
    pub async fn between_days(
        db: &impl ConnectionTrait,
        user_id: i32,
        start: Option<NaiveDate>,
        end: NaiveDate,
    ) -> Result<Vec<Self>, DbErr> {
        let mut query = Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Day.lte(end))
            .order_by_asc(Column::Day);
        if let Some(start) = start {
            query = query.filter(Column::Day.gte(start));
        }
        query.all(db).await
    }
    pub async fn latest_at_or_before(
        db: &impl ConnectionTrait,
        user_id: i32,
        end: NaiveDate,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Day.lte(end))
            .order_by_desc(Column::Day)
            .one(db)
            .await
    }
}

fn fitness_freshness_active_models(
    user_id: i32,
    rows: Vec<FitnessFreshnessDay>,
    rebuilt_at: DateTime<Utc>,
) -> Vec<fitness_freshness_daily::ActiveModel> {
    rows.into_iter()
        .map(|row| fitness_freshness_daily::ActiveModel {
            user_id: Set(user_id),
            day: Set(row.day),
            activity_count: Set(row.activity_count),
            training_load: Set(row.training_load),
            fitness: Set(row.fitness),
            fatigue: Set(row.fatigue),
            form: Set(row.form),
            created_at: Set(rebuilt_at),
            updated_at: Set(rebuilt_at),
            ..Default::default()
        })
        .collect()
}
impl Model {
    pub async fn replace_days(
        db: &impl ConnectionTrait,
        user_id: i32,
        rebuild_from_day: Option<NaiveDate>,
        rows: Vec<FitnessFreshnessDay>,
        rebuilt_at: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        Self::replace_user_rows_from_day(
            db,
            user_id,
            rebuild_from_day,
            fitness_freshness_active_models(user_id, rows, rebuilt_at),
        )
        .await
    }
}
