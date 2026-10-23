use crate::analytics::SegmentUserSummaryAccumulator;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, QuerySelect, Set};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "segment_user_summaries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub segment_id: i32,
    pub user_id: i32,
    #[sea_orm(default_value = 0)]
    pub effort_count: i32,
    pub personal_best_effort_id: Option<i32>,
    pub personal_best_duration_seconds: Option<i32>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::timestamps::impl_timestamps!(ActiveModel);

impl Model {
    /// Existing summaries include segments whose last effort was removed by a
    /// prior matching attempt. Rebuilding this broader scope is safe on retry.
    pub async fn segment_ids_for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Vec<i32>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::SegmentId)
            .filter(Column::UserId.eq(user_id))
            .into_tuple::<i32>()
            .all(db)
            .await
    }

    pub async fn list_by_user_segment_ids<C>(db: &C, user_id: i32, segment_ids: &[i32]) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        if segment_ids.is_empty() {
            return Ok(Vec::new());
        }

        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .all(db)
            .await
    }

    pub async fn list_by_segment_and_user_ids<C>(
        db: &C,
        segment_ids: &[i32],
        user_ids: &[i32],
    ) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        if segment_ids.is_empty() || user_ids.is_empty() {
            return Ok(Vec::new());
        }

        Entity::find()
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .filter(Column::UserId.is_in(user_ids.iter().copied()))
            .all(db)
            .await
    }
}

impl Entity {
    pub async fn delete_for_segment(db: &impl ConnectionTrait, segment_id: i32) -> Result<(), DbErr> {
        Self::delete_many()
            .filter(Column::SegmentId.eq(segment_id))
            .exec(db)
            .await?;
        Ok(())
    }
}

impl Entity {
    pub async fn clear_segments(db: &impl ConnectionTrait, ids: &[i32]) -> Result<sea_orm::DeleteResult, DbErr> {
        use sea_orm::QueryFilter;
        Self::delete_many()
            .filter(Column::SegmentId.is_in(ids.iter().copied()))
            .exec(db)
            .await
    }
}

impl Model {
    pub(crate) async fn store_summaries(
        db: &impl ConnectionTrait,
        summaries: HashMap<(i32, i32), SegmentUserSummaryAccumulator>,
    ) -> Result<(), DbErr> {
        let rows: Vec<_> = summaries
            .into_iter()
            .map(|((segment_id, user_id), summary)| ActiveModel::from_summary(segment_id, user_id, summary))
            .collect();
        for batch in rows.chunks(500) {
            Entity::insert_many(batch.iter().cloned())
                .exec_without_returning(db)
                .await?;
        }
        Ok(())
    }
}

impl ActiveModel {
    fn from_summary(segment_id: i32, user_id: i32, summary: SegmentUserSummaryAccumulator) -> Self {
        Self {
            segment_id: Set(segment_id),
            user_id: Set(user_id),
            effort_count: Set(summary.effort_count),
            personal_best_effort_id: Set(summary.personal_best_effort_id),
            personal_best_duration_seconds: Set(summary.personal_best_duration_seconds),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;

    #[tokio::test]
    async fn rider_summaries_are_inserted_in_bounded_batches_with_their_own_personal_bests() {
        let summaries = (1..=501)
            .map(|user_id| {
                (
                    (1, user_id),
                    SegmentUserSummaryAccumulator {
                        effort_count: user_id,
                        personal_best_effort_id: Some(1000 + user_id),
                        personal_best_duration_seconds: Some(2000 + user_id),
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
        let mut riders = Vec::new();
        let sizes: Vec<_> = log
            .iter()
            .map(|txn| {
                let statement = &txn.statements()[0];
                assert!(statement.sql.starts_with("INSERT INTO \"segment_user_summaries\""));
                assert!(!statement.sql.contains("RETURNING"));
                let values = &statement.values.as_ref().unwrap().0;
                for row in values.as_chunks::<5>().0 {
                    let user_id = match row[1] {
                        sea_orm::Value::Int(Some(id)) => id,
                        _ => panic!("expected rider id"),
                    };
                    assert_eq!(
                        row,
                        &[
                            1.into(),
                            user_id.into(),
                            user_id.into(),
                            (1000 + user_id).into(),
                            (2000 + user_id).into()
                        ]
                    );
                    riders.push(user_id);
                }
                values.len()
            })
            .collect();
        riders.sort_unstable();
        assert_eq!(riders, (1..=501).collect::<Vec<_>>());
        assert_eq!(sizes, vec![2500, 5]);
    }

    #[tokio::test]
    async fn no_rider_summaries_does_not_issue_an_insert() {
        let db = fixtures::postgres_mock().into_connection();
        Model::store_summaries(&db, HashMap::new()).await.unwrap();
        assert!(db.into_transaction_log().is_empty());
    }
}
