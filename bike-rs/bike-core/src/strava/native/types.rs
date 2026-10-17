use super::super::{events::StravaEvent, objects::ObjectRef};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum ActivityStatus {
    #[default]
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "captured")]
    Captured,
    #[sea_orm(string_value = "applied")]
    Applied,
    #[sea_orm(string_value = "unchanged")]
    Unchanged,
    #[sea_orm(string_value = "deleted")]
    Deleted,
    #[sea_orm(string_value = "already_absent")]
    AlreadyAbsent,
    #[sea_orm(string_value = "superseded")]
    Superseded,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivitySource {
    pub event: StravaEvent,
    pub activity: ObjectRef,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivitySources {
    pub event: StravaEvent,
    pub activity: ObjectRef,
    pub streams: ObjectRef,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NormalizedActivity {
    pub event: StravaEvent,
    pub source: ObjectRef,
    pub parsed: Option<ObjectRef>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityChange {
    pub event: StravaEvent,
    pub status: ActivityStatus,
    pub bike_activity_id: Option<i32>,
    pub import_id: Option<i32>,
    pub segment_ids: Vec<i32>,
    pub dirty_from: Option<chrono::NaiveDate>,
    pub removed_source: Option<String>,
}
impl ActivityChange {
    /// Keep dependencies captured with the source mutation, including memberships
    /// removed by an earlier attempt whose completion reply was lost.
    pub fn include_segments(&mut self, segment_ids: impl IntoIterator<Item = i32>) {
        self.segment_ids.extend(segment_ids);
        self.segment_ids.sort_unstable();
        self.segment_ids.dedup();
    }

    pub fn applied(mut event: StravaEvent, activity_id: i32, import_id: i32, dirty_from: chrono::NaiveDate) -> Self {
        event.bike_activity_id = Some(activity_id);
        Self {
            event,
            status: ActivityStatus::Applied,
            bike_activity_id: Some(activity_id),
            import_id: Some(import_id),
            segment_ids: vec![],
            dirty_from: Some(dirty_from),
            removed_source: None,
        }
    }

    pub fn unchanged(event: StravaEvent, status: ActivityStatus) -> Self {
        Self {
            bike_activity_id: event.bike_activity_id,
            event,
            status,
            import_id: None,
            segment_ids: vec![],
            dirty_from: None,
            removed_source: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityWindow {
    pub after: i64,
    pub before: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectionInput {
    pub event: StravaEvent,
    pub window: ActivityWindow,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageInput {
    pub collection: CollectionInput,
    pub page: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChildEvent {
    pub parent: StravaEvent,
    pub child: StravaEvent,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityPage {
    pub activity_ids: Vec<i64>,
    pub next_page: Option<i32>,
    pub latest_started_at: Option<chrono::DateTime<chrono::Utc>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectionResult {
    pub event: StravaEvent,
    pub imported: i32,
    pub unchanged: i32,
    pub last_started_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectionProgress {
    pub next_page: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_page: Option<ActivityPage>,
    pub imported: i32,
    pub unchanged: i32,
    pub last_started_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanRequest {
    pub correlation_id: uuid::Uuid,
    pub after_connection_id: i32,
    #[serde(default)]
    pub failed_customers: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanPage {
    pub events: Vec<StravaEvent>,
    pub next_connection_id: Option<i32>,
}
