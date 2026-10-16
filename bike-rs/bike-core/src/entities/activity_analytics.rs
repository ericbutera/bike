use crate::activity_achievements::StoredActivityAchievementHighlights;
use crate::analytics::ActivityAnalyticsAccumulator;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, Set};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_analytics")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub user_id: i32,
    #[sea_orm(default_value = 0)]
    pub segment_effort_count: i32,
    #[sea_orm(default_value = 0)]
    pub achievement_count: i32,
    #[sea_orm(default_value = 0)]
    pub kom_count: i32,
    #[sea_orm(default_value = 0)]
    pub top_10_count: i32,
    #[sea_orm(default_value = 0)]
    pub pr_count: i32,
    pub achievement_highlights_json: Option<StoredActivityAchievementHighlights>,
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

crate::timestamps::impl_timestamps!(ActiveModel);

impl Model {
    pub async fn delete_by_activity_ids<C>(db: &C, activity_ids: &[i32]) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        if activity_ids.is_empty() {
            return Ok(());
        }

        Entity::delete_many()
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .exec(db)
            .await?;

        Ok(())
    }

    pub async fn list_by_user_activity_ids<C>(
        db: &C,
        user_id: i32,
        activity_ids: &[i32],
    ) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        if activity_ids.is_empty() {
            return Ok(Vec::new());
        }

        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .all(db)
            .await
    }
}

impl Model {
    pub(crate) async fn store_summaries(
        db: &impl ConnectionTrait,
        summaries: HashMap<i32, ActivityAnalyticsAccumulator>,
    ) -> Result<(), DbErr> {
        let rows: Vec<_> = summaries
            .into_iter()
            .filter_map(|(activity_id, analytics)| {
                let user_id = analytics.user_id?;
                Some(ActiveModel::from_summary(activity_id, user_id, analytics))
            })
            .collect();
        // Bound PostgreSQL parameters and avoid one round trip per activity while
        // holding the projection's activity locks. The caller owns the transaction.
        for batch in rows.chunks(500) {
            Entity::insert_many(batch.iter().cloned())
                .exec_without_returning(db)
                .await?;
        }
        Ok(())
    }
}

impl ActiveModel {
    fn from_summary(
        activity_id: i32,
        user_id: i32,
        analytics: ActivityAnalyticsAccumulator,
    ) -> Self {
        Self {
            activity_id: Set(activity_id),
            user_id: Set(user_id),
            segment_effort_count: Set(analytics.segment_effort_count),
            achievement_count: Set(analytics.achievement_count),
            kom_count: Set(analytics.kom_count),
            top_10_count: Set(analytics.top_10_count),
            pr_count: Set(analytics.pr_count),
            achievement_highlights_json: Set(Some(
                StoredActivityAchievementHighlights::from_items(analytics.achievement_highlights),
            )),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;

    #[tokio::test]
    async fn projection_writes_are_batched_instead_of_inserting_each_activity() {
        let summaries = (1..=501)
            .map(|id| {
                (
                    id,
                    ActivityAnalyticsAccumulator {
                        user_id: Some(fixtures::customer().id),
                        segment_effort_count: 1,
                        ..Default::default()
                    },
                )
            })
            .collect();
        let db = fixtures::postgres_mock()
            .append_exec_results([fixtures::written(), fixtures::written()])
            .into_connection();
        Model::store_summaries(&db, summaries).await.unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        let lengths: Vec<_> = log
            .iter()
            .map(|txn| {
                let statement = &txn.statements()[0];
                assert!(statement
                    .sql
                    .starts_with("INSERT INTO \"activity_analytics\""));
                assert!(!statement.sql.contains("RETURNING"));
                statement.values.as_ref().unwrap().0.len()
            })
            .collect();
        assert_eq!(lengths, vec![500 * 8, 8]);
    }

    #[tokio::test]
    async fn projections_without_an_owner_do_not_create_invalid_rows() {
        let db = fixtures::postgres_mock().into_connection();
        Model::store_summaries(
            &db,
            HashMap::from([(42, ActivityAnalyticsAccumulator::default())]),
        )
        .await
        .unwrap();
        assert!(db.into_transaction_log().is_empty());
    }
}
