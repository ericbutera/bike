// OpenAPI re-exports for the background_jobs crate.
// Exposes commonly useful schema types that services may wish to include.

pub mod paths {
    pub use crate::background_jobs::admin::{cancel_task, get_task, list_tasks, rerun_task};

    pub use crate::background_jobs::admin::{
        __path_cancel_task, __path_get_task, __path_list_tasks, __path_rerun_task,
    };
}

pub mod schemas {
    pub use crate::background_jobs::admin::{
        PaginatedResponse, PaginationMetadata, TaskDetailResponse, TaskResponse,
    };
    pub use crate::background_jobs::storage::{TaskRecord, TaskStatus};
}

pub mod tags {
    pub const TAGS: &[(&str, &str)] = &[("tasks", "Background task APIs and task metadata")];
}
