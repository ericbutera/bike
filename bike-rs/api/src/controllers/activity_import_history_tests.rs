use serde_json::json;

#[test]
fn http_input_mapping_keeps_reviewed_replay_preconditions() {
    for expected in [None, Some(("activity_parsed", 10))] {
        let request: super::ReplayRequest = serde_json::from_value(json!({
            "stage": "activity_saved",
            "expected_start_stage": expected.map(|(stage, _)| stage),
            "expected_reused_attempt_id": expected.map(|(_, id)| id),
        }))
        .unwrap();
        let command = crate::activity_import_history::ReplayCommand::from(request);
        assert_eq!(command.stage, "activity_saved");
        assert_eq!(
            command.expected_start_stage.as_deref(),
            expected.map(|(stage, _)| stage)
        );
        assert_eq!(command.expected_reused_attempt_id, expected.map(|(_, id)| id));
    }
}
