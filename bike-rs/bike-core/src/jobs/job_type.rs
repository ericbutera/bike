//! Stable queue names. Convert to strings only at queue and registry boundaries.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    PrepareHeatmap,
    EmailNotification,
    RebuildFitnessFreshness,
    RebuildSegmentAnalytics,
    RegenerateSegmentEfforts,
    ProcessActivityImport,
    ReprocessUserActivityImports,
    ReprocessArchiveFitActivityImports,
    ReprocessActivityImport,
    BackfillUserXcTraining,
    RegenerateUserSegments,
    RegenerateActivitySegments,
    ActivityArchiveImport,
}

impl JobType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PrepareHeatmap => "prepare_heatmap",
            Self::EmailNotification => "email_notification",
            Self::RebuildFitnessFreshness => "rebuild_fitness_freshness",
            Self::RebuildSegmentAnalytics => "rebuild_segment_analytics",
            Self::RegenerateSegmentEfforts => "regenerate_segment_efforts",
            Self::ProcessActivityImport => "process_activity_import",
            Self::ReprocessUserActivityImports => "reprocess_user_activity_imports",
            Self::ReprocessArchiveFitActivityImports => "reprocess_archive_fit_activity_imports",
            Self::ReprocessActivityImport => "reprocess_activity_import",
            Self::BackfillUserXcTraining => "backfill_user_xc_training",
            Self::RegenerateUserSegments => "regenerate_user_segments",
            Self::RegenerateActivitySegments => "regenerate_activity_segments",
            Self::ActivityArchiveImport => "activity_archive_import",
        }
    }
}
