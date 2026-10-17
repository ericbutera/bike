use crate::bike_tasks::{
    ActivityImport, ActivityImportResult, ActivityTask, FitnessChange, ImportSource, ImportTarget, Priority,
    SegmentChange,
};
use crate::object_store::ObjectRef;

pub fn fitness_change() -> FitnessChange {
    FitnessChange {
        user_id: 7,
        priority: Priority::Background,
    }
}

pub fn segment_change() -> SegmentChange {
    SegmentChange {
        segment_id: 18,
        priority: Priority::Background,
    }
}

pub fn activity_task() -> ActivityTask {
    ActivityTask {
        user_id: 7,
        activity_id: 42,
        priority: Priority::NewActivity,
    }
}

pub fn import_command() -> ActivityImport {
    ActivityImport {
        source: ImportSource {
            command_id: "fixture-import-18".into(),
            user_id: 7,
            activity_import_id: 18,
            object: ObjectRef {
                path: "activity-imports/fixture.gpx".into(),
                sha256: "a".repeat(64),
                bytes: 123,
            },
            priority: Priority::NewActivity,
            cause: None,
        },
        target: ImportTarget::NewActivity,
    }
}

pub fn import_result() -> ActivityImportResult {
    ActivityImportResult {
        activity_id: 42,
        activity_import_id: 18,
        user_id: 7,
        priority: Priority::NewActivity,
    }
}

pub fn reprocess_input() -> crate::bike_tasks::ReprocessInput {
    crate::bike_tasks::ReprocessInput {
        source: import_command().source,
        activity_id: 42,
    }
}

pub fn task_cause() -> crate::bike_tasks::TaskCause {
    use crate::bike_tasks::{TaskActor, TaskCause};
    TaskCause {
        correlation_id: "fixture-correlation".into(),
        request_id: Some("fixture-request".into()),
        actor: TaskActor::User(1),
        user_id: 7,
        received_at: super::timestamp(),
        trace_context: std::collections::BTreeMap::from([(
            "traceparent".into(),
            "00-0123456789abcdef0123456789abcdef-0123456789abcdef-01".into(),
        )]),
    }
}

pub fn job_reference() -> crate::bike_tasks::JobReference {
    crate::bike_tasks::JobReference {
        job_id: "unit-workflow".into(),
        namespace: "bike".into(),
        run_id: "00000000-0000-4000-8000-000000000099".into(),
        workflow_type: "process_activity_import".into(),
    }
}

pub fn import_execution() -> crate::bike_tasks::ImportExecution {
    let mut command = import_command();
    command.source.cause = Some(task_cause());
    crate::bike_tasks::ImportExecution {
        command,
        workflow: job_reference(),
    }
}
