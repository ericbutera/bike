//! Current all-time personal podiums from saved segment efforts.
use crate::entities::{segment_efforts, segment_personal_achievements as achievements, segments};
use sea_orm::{ConnectionTrait, DbErr, TransactionSession, TransactionTrait};
use std::collections::HashMap;

fn personal_top_three(mut efforts: Vec<segment_efforts::Model>) -> Vec<achievements::Model> {
    efforts.sort_by_key(|effort| (effort.duration_seconds, effort.id));
    let mut positions = HashMap::<i32, i32>::new();
    efforts
        .into_iter()
        .filter_map(|effort| {
            let position = positions.entry(effort.user_id).or_default();
            *position += 1;
            (*position <= 3).then_some(achievements::Model {
                segment_id: effort.segment_id,
                user_id: effort.user_id,
                position: *position,
                effort_id: effort.id,
                duration_seconds: effort.duration_seconds,
            })
        })
        .collect()
}

/// Imports and the rollout backfill call this same saved-effort operation.
#[tracing::instrument(name = "achievements.personal.rebuild", skip_all, fields(segment_id))]
pub async fn rebuild(db: &(impl ConnectionTrait + TransactionTrait), segment_id: i32) -> Result<(), DbErr> {
    let txn = db.begin().await?;
    let Some(segment) = segments::Model::lock_for_analytics(&txn, segment_id).await? else {
        return Ok(());
    };
    let efforts = segment_efforts::Model::list_for_segment_analytics(&txn, &[segment.id]).await?;
    achievements::Model::replace_for_segment(&txn, segment.id, personal_top_three(efforts)).await?;
    txn.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;
    use sea_orm::{DbErr, MockDatabase};
    use std::collections::BTreeMap;

    fn effort(id: i32, user_id: i32, duration_seconds: i32) -> segment_efforts::Model {
        segment_efforts::Model {
            id,
            user_id,
            duration_seconds,
            ..fixtures::segment_effort()
        }
    }

    fn snapshot(efforts: Vec<segment_efforts::Model>) -> MockDatabase {
        fixtures::postgres_mock()
            .append_query_results([vec![fixtures::segment()]])
            .append_query_results([efforts])
    }

    #[test]
    fn every_rider_gets_their_own_pr_and_up_to_three_individual_efforts() {
        let rows = personal_top_three(vec![
            effort(5, 7, 500),
            effort(2, 7, 200),
            effort(4, 7, 400),
            effort(1, 7, 100),
            effort(3, 7, 300),
            effort(6, 8, 900),
            effort(7, 8, 950),
        ]);
        let personal: Vec<_> = rows
            .iter()
            .filter(|row| row.user_id == 7)
            .map(|row| (row.position, row.effort_id))
            .collect();
        let other: Vec<_> = rows
            .iter()
            .filter(|row| row.user_id == 8)
            .map(|row| (row.position, row.effort_id))
            .collect();
        assert_eq!(personal, vec![(1, 1), (2, 2), (3, 3)]);
        assert_eq!(other, vec![(1, 6), (2, 7)]);
        assert!(rows
            .iter()
            .all(|row| row.segment_id == fixtures::segment_effort().segment_id));
    }

    #[test]
    fn equal_times_use_effort_identity_and_deletion_promotes_the_next_attempt() {
        let rows = personal_top_three(vec![effort(4, 7, 400), effort(3, 7, 100), effort(2, 7, 100)]);
        assert_eq!(rows.iter().map(|row| row.effort_id).collect::<Vec<_>>(), vec![2, 3, 4]);
        let after_delete = personal_top_three(vec![effort(3, 7, 100), effort(4, 7, 400)]);
        assert_eq!(after_delete[0].effort_id, 3);
        assert_eq!(after_delete[0].position, 1);
        assert_eq!(after_delete[1].position, 2);
    }

    #[tokio::test]
    async fn rebuild_reads_saved_efforts_and_replaces_personal_positions_in_one_transaction() {
        let db = snapshot(vec![effort(1, 7, 100), effort(2, 8, 900)])
            .append_exec_results([fixtures::written(), fixtures::written()])
            .into_connection();
        rebuild(&db, fixtures::segment().id).await.unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        let statements = log[0].statements();
        assert_eq!(statements[0].sql, "BEGIN");
        assert!(statements[1].sql.ends_with("FOR UPDATE"));
        assert!(statements[2].sql.contains("FROM \"segment_efforts\""));
        assert!(statements[3]
            .sql
            .starts_with("DELETE FROM \"segment_personal_achievements\""));
        assert!(statements[4]
            .sql
            .starts_with("INSERT INTO \"segment_personal_achievements\""));
        assert_eq!(
            statements[4].values.as_ref().unwrap().0,
            vec![
                1.into(),
                7.into(),
                1.into(),
                1.into(),
                100.into(),
                1.into(),
                8.into(),
                1.into(),
                2.into(),
                900.into()
            ]
        );
        assert_eq!(statements.last().unwrap().sql, "COMMIT");
    }

    #[tokio::test]
    async fn an_empty_segment_clears_old_personal_positions() {
        let db = snapshot(Vec::new())
            .append_exec_results([fixtures::written()])
            .into_connection();
        rebuild(&db, fixtures::segment().id).await.unwrap();
        let log = db.into_transaction_log();
        let statements = log[0].statements();
        assert!(statements.iter().any(|statement| statement.sql.starts_with("DELETE")));
        assert!(!statements.iter().any(|statement| statement.sql.starts_with("INSERT")));
        assert_eq!(statements.last().unwrap().sql, "COMMIT");
    }

    #[tokio::test]
    async fn rerunning_processing_replaces_the_same_rows_without_duplicates() {
        let db = snapshot(vec![effort(1, 7, 100)])
            .append_query_results([vec![fixtures::segment()]])
            .append_query_results([vec![effort(1, 7, 100)]])
            .append_exec_results((0..4).map(|_| fixtures::written()))
            .into_connection();
        rebuild(&db, 1).await.unwrap();
        rebuild(&db, 1).await.unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        let first = log[0].statements();
        let second = log[1].statements();
        assert_eq!(first, second);
        assert!(second[3].sql.starts_with("DELETE"));
        assert!(second[4].sql.starts_with("INSERT"));
        assert_eq!(second.last().unwrap().sql, "COMMIT");
    }

    #[tokio::test]
    async fn publication_failure_rolls_back_the_entire_replacement() {
        let db = snapshot(vec![effort(1, 7, 100)])
            .append_exec_results([fixtures::written()])
            .append_exec_errors([DbErr::Custom("fixture insert failed".into())])
            .into_connection();
        assert!(rebuild(&db, fixtures::segment().id).await.is_err());
        let log = db.into_transaction_log();
        assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
    }

    #[tokio::test]
    async fn personal_reads_return_saved_positions_without_waiting_for_refresh() {
        let rows = personal_top_three(vec![effort(1, 7, 100), effort(2, 7, 200)]);
        let db = fixtures::postgres_mock()
            .append_query_results([rows.clone()])
            .into_connection();
        assert_eq!(achievements::Model::for_user_segment(&db, 7, 1).await.unwrap(), rows);
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        let query = &log[0].statements()[0];
        assert!(query.sql.contains("\"segment_id\" = $1 AND"));
        assert!(query.sql.contains("\"user_id\" = $2"));
        assert!(query.sql.contains("\"position\" ASC"));
        assert!(!query.sql.contains("segment_personal_achievement_outputs"));
        assert!(!query.sql.contains("segment_efforts"));
        assert_eq!(query.values.as_ref().unwrap().0, vec![1.into(), 7.into()]);
    }

    #[tokio::test]
    async fn personal_reads_without_saved_positions_are_empty() {
        let db = fixtures::postgres_mock()
            .append_query_results([Vec::<achievements::Model>::new()])
            .into_connection();
        assert!(achievements::Model::for_user_segment(&db, 7, 1)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(db.into_transaction_log().len(), 1);
    }

    #[tokio::test]
    async fn a_deleted_segment_needs_no_output() {
        let db = fixtures::postgres_mock()
            .append_query_results([Vec::<segments::Model>::new()])
            .into_connection();
        rebuild(&db, 1).await.unwrap();
        let log = db.into_transaction_log();
        let statements = log[0].statements();
        assert!(!statements.iter().any(|statement| statement.sql.starts_with("DELETE")));
        assert!(!statements.iter().any(|statement| statement.sql.starts_with("INSERT")));
    }

    #[tokio::test]
    async fn backfill_pages_all_segment_ids_in_order() {
        let row: BTreeMap<String, sea_orm::Value> = [("id".into(), 17.into())].into();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![row]])
            .into_connection();
        assert_eq!(segments::Model::ids_page(&db, 16, 16).await.unwrap(), vec![17]);
        let log = db.into_transaction_log();
        let query = &log[0].statements()[0];
        assert!(query.sql.contains("\"segments\".\"id\" > $1"));
        assert!(query.sql.contains("ORDER BY \"segments\".\"id\" ASC LIMIT $2"));
        assert_eq!(query.values.as_ref().unwrap().0, vec![16.into(), 16_u64.into()]);
    }
}
