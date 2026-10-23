use super::{Column, Entity, Model, RankUpdate};
use sea_orm::sea_query::{CaseStatement, Expr};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};

impl Model {
    pub(crate) async fn set_ranks_batch(db: &impl ConnectionTrait, updates: &[RankUpdate]) -> Result<(), DbErr> {
        // Bound SQL size and PostgreSQL parameters. The analytics caller retains
        // its segment lock and transaction across every batch.
        for batch in updates.chunks(500) {
            let mut overall_ranks = CaseStatement::new();
            let mut user_ranks = CaseStatement::new();
            for update in batch {
                overall_ranks = overall_ranks.case(Column::Id.eq(update.effort_id), update.overall_rank);
                user_ranks = user_ranks.case(Column::Id.eq(update.effort_id), update.user_rank);
            }
            let result = Entity::update_many()
                .col_expr(
                    Column::OverallRank,
                    overall_ranks.finally(Expr::col(Column::OverallRank)).into(),
                )
                .col_expr(Column::UserRank, user_ranks.finally(Expr::col(Column::UserRank)).into())
                .col_expr(Column::UpdatedAt, Expr::current_timestamp())
                .filter(Column::Id.is_in(batch.iter().map(|update| update.effort_id)))
                .exec(db)
                .await?;
            if result.rows_affected != batch.len() as u64 {
                return Err(DbErr::RecordNotFound(
                    "segment effort missing during rank refresh".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;
    use sea_orm::MockExecResult;

    #[tokio::test]
    async fn effort_ranks_are_updated_in_bounded_batches_without_replacing_efforts() {
        let updates: Vec<_> = (1..=501)
            .map(|id| RankUpdate {
                effort_id: id,
                overall_rank: id,
                user_rank: 502 - id,
            })
            .collect();
        let db = fixtures::postgres_mock()
            .append_exec_results([500, 1].map(|rows_affected| MockExecResult {
                rows_affected,
                ..Default::default()
            }))
            .into_connection();
        Model::set_ranks_batch(&db, &updates).await.unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        let sizes: Vec<_> = log
            .iter()
            .map(|txn| {
                let statement = &txn.statements()[0];
                assert!(statement
                    .sql
                    .starts_with("UPDATE \"segment_efforts\" SET \"overall_rank\" = "));
                assert!(statement.sql.contains("\"user_rank\" = "));
                assert!(statement.sql.contains("\"updated_at\" = CURRENT_TIMESTAMP"));
                assert!(statement.sql.contains("WHERE \"segment_efforts\".\"id\" IN"));
                assert!(!statement.sql.contains("RETURNING"));
                statement.values.as_ref().unwrap().0.len()
            })
            .collect();
        assert_eq!(sizes, vec![2500, 5]);
        let last = &log[1].statements()[0];
        assert_eq!(
            last.values.as_ref().unwrap().0,
            vec![501.into(), 501.into(), 501.into(), 1.into(), 501.into()]
        );
    }

    #[tokio::test]
    async fn missing_efforts_fail_rank_persistence_instead_of_silently_completing() {
        let db = fixtures::postgres_mock()
            .append_exec_results([MockExecResult::default()])
            .into_connection();
        let error = Model::set_ranks_batch(
            &db,
            &[RankUpdate {
                effort_id: 99,
                overall_rank: 1,
                user_rank: 1,
            }],
        )
        .await
        .unwrap_err();
        assert!(matches!(error, DbErr::RecordNotFound(_)));
    }

    #[tokio::test]
    async fn no_effort_ranks_does_not_issue_an_update() {
        let db = fixtures::postgres_mock().into_connection();
        Model::set_ranks_batch(&db, &[]).await.unwrap();
        assert!(db.into_transaction_log().is_empty());
    }
}
