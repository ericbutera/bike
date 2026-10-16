use super::batches::page;
use super::*;
use crate::workflows::BackfillUserXcTraining;
use bike_core::{bike_tasks::*, test_support as fixtures};

#[test]
fn training_backfill_reuses_fitness_handoff_and_completes_after_all_activity_work() {
    let input = fixtures::batch_input();
    let request = fixtures::page_request(ManifestScope::Training { user_id: 7 }, 2, 0);
    let mut mock = WorkflowMock::new::<BackfillUserXcTraining>(input.clone());
    mock.activity(
        "prepare_training_backfill",
        &input.command,
        &request.manifest,
    );
    let entries = page(&request, fixtures::training_entry);
    mock.activity("training_activity_page", &request, &entries);
    mock.activity(
        "rebuild_activity_training",
        &entries[0],
        &fixtures::training_result(),
    );
    mock.activity(
        "schedule_fitness_freshness",
        &fixtures::fitness_change(),
        &(),
    );
    mock.activity(
        "rebuild_activity_training",
        &entries[1],
        &TrainingActivityResult { fitness: None },
    );
    let result = BatchResult {
        command_id: input.command.command_id.clone(),
        completed: 2,
    };
    mock.activity(
        "complete_training_backfill",
        &TrainingCompletion {
            command: input.command,
            result: result.clone(),
        },
        &(),
    );
    assert_eq!(mock.result::<BatchResult>(), result);
}

#[test]
fn training_failure_does_not_mark_the_backfill_complete() {
    let request = fixtures::page_request(ManifestScope::Training { user_id: 7 }, 1, 0);
    let mut mock = WorkflowMock::new::<BackfillUserXcTraining>(fixtures::prepared_batch(&request));
    mock.activity(
        "training_activity_page",
        &request,
        &page(&request, fixtures::training_entry),
    );
    mock.fail_activity("rebuild_activity_training", "training failed");
    mock.assert_failure_message("training failed");
}

#[test]
fn training_cancellation_does_not_mark_the_backfill_complete() {
    let request = fixtures::page_request(ManifestScope::Training { user_id: 7 }, 1, 0);
    let mut mock = WorkflowMock::new::<BackfillUserXcTraining>(fixtures::prepared_batch(&request));
    mock.activity(
        "training_activity_page",
        &request,
        &page(&request, fixtures::training_entry),
    );
    mock.cancel_activity("rebuild_activity_training");
    mock.assert_cancelled();
}
