use super::*;
use crate::test_support as fixtures;
use sea_orm::{DbBackend, MockDatabase, MockExecResult, Value};

fn written(rows_affected: u64) -> MockExecResult {
    MockExecResult {
        rows_affected,
        ..Default::default()
    }
}

#[tokio::test]
async fn fitness_invalidations_merge_first_duplicate_and_out_of_order_changes_in_one_write() {
    let state = fixtures::analytics_user_state();
    let day = state.updated_at.date_naive();
    let requests = [
        (day, state.updated_at),
        (
            day - chrono::Duration::days(30),
            state.updated_at - chrono::Duration::days(1),
        ),
        (
            day + chrono::Duration::days(1),
            state.updated_at + chrono::Duration::days(1),
        ),
        (day, state.updated_at),
    ];
    // No query results: even the first invalidation must not read before writing.
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_exec_results(requests.iter().map(|_| written(1)))
        .into_connection();
    for (dirty_from, changed_at) in requests {
        Model::mark_dirty(&db, state.user_id, dirty_from, changed_at)
            .await
            .unwrap();
    }
    let transactions = db.into_transaction_log();
    let statements = transactions
        .iter()
        .flat_map(|txn| txn.statements())
        .collect::<Vec<_>>();
    assert_eq!(statements.len(), requests.len());
    for (statement, (dirty_from, changed_at)) in statements.iter().zip(requests) {
        let update = statement.sql.split_once("DO UPDATE SET").unwrap().1;
        assert!(statement.sql.contains("ON CONFLICT (\"user_id\")"));
        assert!(
            update.contains("LEAST(COALESCE(\"analytics_user_states\".\"fitness_dirty_from_day\"")
        );
        assert!(update.contains("GREATEST(\"analytics_user_states\".\"last_activity_change_at\""));
        assert!(update.contains("\"updated_at\" = \"excluded\".\"updated_at\""));
        assert!(!update.contains("created_at"));
        assert!(!update.contains("last_fitness_rebuild_at"));
        let values = &statement.values.as_ref().unwrap().0;
        for expected in [
            Value::from(state.user_id),
            dirty_from.into(),
            changed_at.into(),
        ] {
            assert!(values.contains(&expected));
        }
    }
}

#[tokio::test]
async fn fitness_invalidation_advances_its_write_fence_even_for_an_old_source_timestamp() {
    let state = fixtures::analytics_user_state();
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_exec_results([written(1)])
        .into_connection();
    let started_at = Utc::now();
    Model::mark_dirty(
        &db,
        state.user_id,
        state.updated_at.date_naive(),
        state.updated_at,
    )
    .await
    .unwrap();
    let finished_at = Utc::now();
    let log = db.into_transaction_log();
    let values = &log[0].statements()[0].values.as_ref().unwrap().0;
    assert!(values.iter().any(|value| {
        matches!(value, Value::ChronoDateTimeUtc(Some(at)) if *at >= started_at && *at <= finished_at)
    }));
}

#[tokio::test]
async fn fitness_rebuild_completion_keeps_source_and_write_fences() {
    let state = Model {
        fitness_dirty_from_day: Some(fixtures::timestamp().date_naive()),
        ..fixtures::analytics_user_state()
    };
    for rows_affected in [0, 1] {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_exec_results([written(rows_affected)])
            .into_connection();
        assert_eq!(
            Model::complete_rebuild(&db, state.user_id, Some(state.clone()), state.updated_at)
                .await
                .unwrap(),
            rows_affected == 1
        );
        let log = db.into_transaction_log();
        let statement = &log[0].statements()[0];
        let predicate = statement.sql.split_once("WHERE").unwrap().1;
        assert!(predicate.contains("\"user_id\" ="));
        assert!(predicate.contains("\"last_activity_change_at\" ="));
        assert!(predicate.contains("\"updated_at\" ="));
    }
}

#[tokio::test]
async fn fitness_invalidation_returns_database_failure() {
    let state = fixtures::analytics_user_state();
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_exec_errors([DbErr::Custom("fixture write failed".into())])
        .into_connection();
    let error = Model::mark_dirty(
        &db,
        state.user_id,
        state.updated_at.date_naive(),
        state.updated_at,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("fixture write failed"));
}
