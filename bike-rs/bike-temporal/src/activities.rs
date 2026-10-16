//! Domain adapters are grouped by effect ownership. None may enqueue SQL tasks.
mod archive;
mod derived;
mod imports;
mod segments;
mod training;
pub use archive::{ArchiveActivities, ArchiveOperations};
pub use derived::{DerivedActivities, DerivedOperations};
pub use imports::{ImportActivities, ImportOperations};
pub use segments::{SegmentActivities, SegmentOperations};
pub use training::{TrainingActivities, TrainingOperations};

pub(crate) fn activity_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> temporalio_sdk::activities::ActivityError {
    temporalio_sdk::ApplicationFailure::new(error).into()
}
