use super::*;
use crate::activity_sport::ActivitySport;
use crate::test_support::{self as fixtures, temporal_client};
use sea_orm::{DbBackend, MockDatabase, MockExecResult};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn editing_sport_submits_existing_projection_workflows_after_the_manual_write() {
    for changed_sport in [None, Some(ActivitySport::Hike)] {
        let before = fixtures::activity();
        let after = activities::Model {
            title: "Saved title".into(),
            sport: changed_sport.map_or_else(|| before.sport.clone(), |sport| sport.as_str().into()),
            estimated_ftp_watts: None,
            ..before.clone()
        };
        let writes = if changed_sport.is_some() { 5 } else { 1 };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![before.clone()], vec![after.clone()]])
            .append_exec_results((0..writes).map(|_| MockExecResult {
                rows_affected: 1,
                ..Default::default()
            }))
            .into_connection();
        let requests = Arc::new(Mutex::new(temporal_client::Requests::default()));
        let tasks = TaskQueue::with_client(db.clone(), temporal_client::client(requests.clone()).await);
        let updated = update_activity_details(
            &db,
            &tasks,
            before,
            activities::ActivityDetailsUpdate {
                title: Some("Saved title".into()),
                sport: changed_sport,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(updated, after);
        let requests = requests.lock().unwrap();
        let workflows = requests
            .starts
            .iter()
            .map(|request| request.workflow_type.as_ref().unwrap().name.as_str())
            .collect::<Vec<_>>();
        let expected = if changed_sport.is_some() {
            vec![
                "regenerate_activity_segments",
                "rebuild_fitness_freshness",
                "prepare_heatmap",
            ]
        } else {
            vec!["prepare_heatmap"]
        };
        assert_eq!(workflows, expected);
        for request in &requests.starts {
            let payload = &request.input.as_ref().unwrap().payloads[0].data;
            let parameters: serde_json::Value = serde_json::from_slice(payload).unwrap();
            assert_eq!(parameters["user_id"], after.user_id);
        }
        drop(tasks);
        let log = db.into_transaction_log();
        assert_eq!(log[0].statements().last().unwrap().sql, "COMMIT");
    }
}
