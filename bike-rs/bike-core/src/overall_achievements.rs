//! Current all-time global standings, with one position per athlete.
use crate::entities::{segment_efforts, segment_overall_achievements as achievements, segments};
use sea_orm::{ConnectionTrait, DbErr, TransactionSession, TransactionTrait};
use std::collections::HashSet;

fn overall_top_ten(mut efforts: Vec<segment_efforts::Model>) -> Vec<achievements::Model> {
    efforts.sort_by_key(|effort| (effort.duration_seconds, effort.id));
    let mut athletes = HashSet::new();
    efforts
        .into_iter()
        .filter(|effort| athletes.insert(effort.user_id))
        .take(10)
        .zip(1..=10)
        .map(|(effort, position)| achievements::Model {
            segment_id: effort.segment_id,
            position,
            user_id: effort.user_id,
            effort_id: effort.id,
            duration_seconds: effort.duration_seconds,
        })
        .collect()
}

/// Analytics and the rollout backfill call the same saved-effort operation.
#[tracing::instrument(name = "achievements.overall.rebuild", skip_all, fields(segment_id))]
pub async fn rebuild(db: &(impl ConnectionTrait + TransactionTrait), segment_id: i32) -> Result<(), DbErr> {
    let txn = db.begin().await?;
    let Some(segment) = segments::Model::lock_for_analytics(&txn, segment_id).await? else {
        return Ok(());
    };
    let efforts = segment_efforts::Model::list_for_segment_analytics(&txn, &[segment.id]).await?;
    achievements::Model::replace_for_segment(&txn, segment.id, overall_top_ten(efforts)).await?;
    txn.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;
    use sea_orm::MockDatabase;

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
    fn repeated_athletes_occupy_one_position_using_their_fastest_effort() {
        let mut efforts: Vec<_> = (1..=12).map(|id| effort(id, id, 100 + id)).collect();
        efforts.extend([effort(20, 1, 50), effort(21, 1, 60), effort(22, 2, 70)]);
        efforts.reverse();
        let rows = overall_top_ten(efforts);
        assert_eq!(rows.len(), 10);
        assert_eq!(
            rows.iter().map(|row| row.position).collect::<Vec<_>>(),
            (1..=10).collect::<Vec<_>>()
        );
        assert_eq!(
            rows.iter().map(|row| row.user_id).collect::<Vec<_>>(),
            (1..=10).collect::<Vec<_>>()
        );
        assert_eq!(rows[0].effort_id, 20);
        assert_eq!(rows[1].effort_id, 22);
    }

    #[test]
    fn equal_times_use_effort_identity_and_each_athlete_gets_a_distinct_position() {
        let rows = overall_top_ten(vec![effort(4, 8, 100), effort(3, 7, 100), effort(2, 7, 100)]);
        assert_eq!(
            rows.iter().map(|row| (row.position, row.effort_id)).collect::<Vec<_>>(),
            vec![(1, 2), (2, 4)]
        );
    }

    #[test]
    fn deletion_promotes_the_athletes_next_attempt_and_the_next_qualifying_athlete() {
        let mut efforts: Vec<_> = (2..=11).map(|id| effort(id, id, id * 100)).collect();
        efforts.push(effort(12, 1, 350));
        let rows = overall_top_ten(efforts.clone());
        assert_eq!(rows[0].user_id, 2);
        assert_eq!(rows[2].effort_id, 12);
        efforts.retain(|effort| effort.user_id != 1);
        let rows = overall_top_ten(efforts);
        assert_eq!(rows[9].user_id, 11);
        assert_eq!(rows[9].position, 10);
    }

    #[tokio::test]
    async fn rebuild_locks_the_segment_and_atomically_replaces_standings() {
        let db = snapshot(vec![effort(1, 7, 100), effort(2, 7, 200), effort(3, 8, 300)])
            .append_exec_results((0..2).map(|_| fixtures::written()))
            .into_connection();
        rebuild(&db, 1).await.unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        let statements = log[0].statements();
        assert_eq!(statements[0].sql, "BEGIN");
        assert!(statements[1].sql.ends_with("FOR UPDATE"));
        assert!(statements[2].sql.contains("FROM \"segment_efforts\""));
        assert!(statements[3]
            .sql
            .starts_with("DELETE FROM \"segment_overall_achievements\""));
        assert!(statements[4]
            .sql
            .starts_with("INSERT INTO \"segment_overall_achievements\""));
        assert_eq!(
            statements[4].values.as_ref().unwrap().0,
            vec![
                1.into(),
                1.into(),
                7.into(),
                1.into(),
                100.into(),
                1.into(),
                2.into(),
                8.into(),
                3.into(),
                300.into()
            ]
        );
        assert_eq!(statements.last().unwrap().sql, "COMMIT");
    }

    #[tokio::test]
    async fn empty_segments_clear_old_standings() {
        let db = snapshot(Vec::new())
            .append_exec_results([fixtures::written()])
            .into_connection();
        rebuild(&db, 1).await.unwrap();
        let log = db.into_transaction_log();
        let statements = log[0].statements();
        assert!(statements[3].sql.starts_with("DELETE"));
        assert_eq!(statements.last().unwrap().sql, "COMMIT");
        assert_eq!(statements.len(), 5);
    }

    #[tokio::test]
    async fn retries_replace_the_same_rows() {
        let db = snapshot(vec![effort(1, 7, 100)])
            .append_query_results([vec![fixtures::segment()]])
            .append_query_results([vec![effort(1, 7, 100)]])
            .append_exec_results((0..4).map(|_| fixtures::written()))
            .into_connection();
        rebuild(&db, 1).await.unwrap();
        rebuild(&db, 1).await.unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].statements(), log[1].statements());
    }

    #[tokio::test]
    async fn failed_publication_rolls_back() {
        let db = snapshot(vec![effort(1, 7, 100)])
            .append_exec_results([fixtures::written()])
            .append_exec_errors([DbErr::Custom("fixture insert failure".into())])
            .into_connection();
        assert!(rebuild(&db, 1).await.is_err());
        let log = db.into_transaction_log();
        assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
    }

    #[tokio::test]
    async fn reads_return_saved_rows_or_empty_without_reading_efforts_or_checking_freshness() {
        let rows = overall_top_ten(vec![effort(1, 7, 100)]);
        for saved in [rows, Vec::new()] {
            let db = fixtures::postgres_mock()
                .append_query_results([saved.clone()])
                .into_connection();
            assert_eq!(achievements::Model::for_segment(&db, 1).await.unwrap(), saved);
            let log = db.into_transaction_log();
            assert_eq!(log.len(), 1);
            let statements = log[0].statements();
            assert_eq!(statements.len(), 1);
            assert!(statements[0]
                .sql
                .contains("ORDER BY \"segment_overall_achievements\".\"position\" ASC"));
            assert!(!statements[0].sql.contains("segment_efforts"));
            assert_eq!(statements[0].values.as_ref().unwrap().0, vec![1.into()]);
        }
    }

    #[tokio::test]
    async fn a_deleted_segment_needs_no_output() {
        let db = fixtures::postgres_mock()
            .append_query_results([Vec::<segments::Model>::new()])
            .into_connection();
        rebuild(&db, 1).await.unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log[0].statements().len(), 3);
    }
}
