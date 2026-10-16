use crate::bike_tasks::{
    ActivityImport, ActivityImportResult, FitnessChange, HeatmapChange, ImportSource, ImportTarget,
    Priority, SegmentChange,
};
use crate::bike_tasks::{
    ActivitySegmentsInput, ActivitySegmentsResult, ArchiveInput, BatchCommand, BatchInput,
    BatchManifest, BatchProgress, ManifestScope, PageRequest, ReprocessInput, SegmentEffortsInput,
    SegmentSelection, TrainingActivityInput, TrainingActivityResult,
};
use crate::object_store::ObjectRef;

pub fn fitness_change() -> FitnessChange {
    FitnessChange {
        user_id: 7,
        revision: 1,
        dirty_from: super::timestamp().date_naive(),
        priority: Priority::Background,
    }
}

pub fn segment_change() -> SegmentChange {
    SegmentChange {
        segment_id: 18,
        revision: 1,
        priority: Priority::Background,
    }
}

pub fn heatmap_change() -> HeatmapChange {
    HeatmapChange {
        user_id: 7,
        activity_id: 42,
        generation: 1,
        priority: Priority::Background,
    }
}

pub fn import_command() -> ActivityImport {
    ActivityImport {
        source: ImportSource {
            command_id: "fixture-import-18".into(),
            user_id: 7,
            activity_import_id: 18,
            object: ObjectRef {
                path: "bike/tasks/sources/fixture.gpx".into(),
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
        fitness: fitness_change(),
        segments: vec![segment_change()],
        heatmap: Some(heatmap_change()),
    }
    .with_priority(Priority::NewActivity)
}

pub fn batch_command() -> BatchCommand {
    BatchCommand {
        command_id: "fixture-batch".into(),
        user_id: 7,
        priority: Priority::Background,
    }
}

pub fn batch_input() -> BatchInput {
    BatchInput::new(batch_command())
}

pub fn batch_manifest(command: &BatchCommand, scope: ManifestScope, entries: u64) -> BatchManifest {
    BatchManifest {
        command_id: command.command_id.clone(),
        scope,
        entries,
        object: ObjectRef {
            path: "bike/tasks/manifests/fixture.json".into(),
            sha256: "b".repeat(64),
            bytes: 123,
        },
    }
}

pub fn page_request(scope: ManifestScope, entries: u64, offset: u64) -> PageRequest {
    let command = batch_command();
    PageRequest {
        manifest: batch_manifest(&command, scope, entries),
        command,
        offset,
    }
}

pub fn prepared_batch(page: &PageRequest) -> BatchInput {
    BatchInput {
        command: page.command.clone(),
        progress: BatchProgress {
            manifest: Some(page.manifest.clone()),
            settled: page.offset,
        },
    }
}

pub fn import_entry(page: &PageRequest, index: u64) -> ImportSource {
    ImportSource {
        command_id: format!("{}:entry-{index}", page.command.command_id),
        user_id: page.command.user_id,
        activity_import_id: 18 + i32::try_from(index).expect("fixture index fits i32"),
        priority: page.command.priority,
        ..import_command().source
    }
}

pub fn reprocess_entry(page: &PageRequest, index: u64) -> ReprocessInput {
    ReprocessInput {
        source: import_entry(page, index),
        activity_id: 42 + i32::try_from(index).expect("fixture index fits i32"),
        expected_revision: 1,
    }
}

pub fn segment_entry(page: &PageRequest, index: u64) -> ActivitySegmentsInput {
    let selection = match page.manifest.scope {
        ManifestScope::Segment {
            segment_id,
            expected_revision,
        } => SegmentSelection::One {
            segment_id,
            expected_revision,
        },
        _ => SegmentSelection::All,
    };
    ActivitySegmentsInput {
        command: BatchCommand {
            command_id: format!("{}:entry-{index}", page.command.command_id),
            ..page.command.clone()
        },
        activity_id: 42 + i32::try_from(index).expect("fixture index fits i32"),
        expected_revision: 1,
        selection,
    }
}

pub fn segment_result() -> ActivitySegmentsResult {
    ActivitySegmentsResult {
        activity_id: 42,
        segments: vec![segment_change()],
    }
}

pub fn segment_efforts_input() -> SegmentEffortsInput {
    SegmentEffortsInput {
        segment_id: 18,
        expected_revision: 1,
        batch: batch_input(),
    }
}

pub fn training_entry(page: &PageRequest, index: u64) -> TrainingActivityInput {
    let segment = segment_entry(page, index);
    TrainingActivityInput {
        command: segment.command,
        activity_id: segment.activity_id,
        expected_revision: 1,
    }
}

pub fn training_result() -> TrainingActivityResult {
    TrainingActivityResult {
        fitness: Some(fitness_change()),
    }
}

pub fn archive_input() -> ArchiveInput {
    ArchiveInput {
        archive_job_id: 9,
        source: ObjectRef {
            path: "bike/tasks/archives/fixture.zip".into(),
            sha256: "c".repeat(64),
            bytes: 123,
        },
        batch: batch_input(),
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
