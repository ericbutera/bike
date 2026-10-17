use super::{imports::complete_derived_tasks, *};
use bike_core::test_support as fixtures;

#[test]
fn archive_calls_processing_which_awaits_its_derived_subtasks() {
    let source = ImportSource {
        priority: Priority::Background,
        ..fixtures::import_command().source
    };
    let imported = ActivityImportResult {
        priority: source.priority,
        ..fixtures::import_result()
    };
    let cursor = ArchiveCursor {
        job_id: 7,
        archive_path: "/uploads/archives/7.zip".into(),
        offset: 0,
    };
    let mut archive = WorkflowMock::new::<ActivityArchiveImport>(ArchiveInput {
        job_id: cursor.job_id,
        cursor: Some(cursor.clone()),
    });
    archive.activity(
        "archive_page",
        &cursor,
        &ArchivePage {
            sources: vec![source.clone()],
            next_offset: 1,
            done: true,
        },
    );
    let child = archive.start_child("process_activity_import", &source);
    archive.assert_pending();
    let mut process = WorkflowMock::new::<ProcessActivityImport>(source.clone());
    process.activity(
        "activity_import",
        &ActivityImport {
            source,
            target: ImportTarget::NewActivity,
        },
        &imported,
    );
    complete_derived_tasks(&mut process, &imported);
    archive.complete_child(child, &process.result::<ActivityImportResult>());
    archive.activity("finish_archive", &cursor.job_id, &());
    archive.result::<()>();
}

#[test]
fn user_batch_calls_reprocessing_which_preserves_identity_through_its_subtasks() {
    let mut input = fixtures::reprocess_input();
    input.source.priority = Priority::Background;
    let imported = ActivityImportResult {
        priority: Priority::Background,
        ..fixtures::import_result()
    };
    let page = UserPage {
        user_id: input.source.user_id,
        after_id: 0,
    };
    let mut batch = WorkflowMock::new::<ReprocessUserActivityImports>(page.clone());
    batch.activity(
        "activity_page",
        &ActivityPage {
            page,
            selection: ActivitySelection::Imports {
                archive_fits_only: false,
            },
        },
        &vec![imported.activity()],
    );
    batch.activity("reprocess_input", &imported.activity(), &input);
    let child = batch.start_child("reprocess_activity_import", &input);
    batch.assert_pending();
    let mut reprocess = WorkflowMock::new::<ReprocessActivityImport>(input.clone());
    reprocess.activity("activity_import", &input.into_command(), &imported);
    complete_derived_tasks(&mut reprocess, &imported);
    let result = reprocess.result::<ActivityImportResult>();
    assert_eq!(result, imported);
    batch.complete_child(child, &result);
    batch.result::<()>();
}
