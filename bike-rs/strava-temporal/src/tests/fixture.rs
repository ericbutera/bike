use bike_core::strava::{
    events::{StravaEvent, StravaOperation},
    native::*,
    objects::{checksum, ObjectRef},
};
use serde_json::json;

pub struct Fixture {
    pub event: StravaEvent,
    pub activity_id: i64,
}
impl Fixture {
    pub fn new(operation: StravaOperation) -> Self {
        let activity: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/activity.json")).unwrap();
        let event = serde_json::from_value(json!({
            "strava_event_id":"00000000-0000-4000-8000-000000000001",
            "correlation_id":"00000000-0000-4000-8000-000000000001",
            "bike_customer_id":"00000000-0000-4000-8000-000000000002",
            "bike_user_id":7, "bike_activity_id":null,
            "strava_athlete_id":123, "strava_activity_id":if matches!(operation, StravaOperation::Sync | StravaOperation::Backfill | StravaOperation::Disconnect) { serde_json::Value::Null } else { activity["id"].clone() },
            "connection_id":9, "connection_generation":"2026-09-01T00:00:00Z",
            "operation":operation, "event_time":1790856000,
            "webhook":null,"request_id":"fixture-request","trace_context":{
                "traceparent":"00-12345678901234567890123456789012-1234567890123456-01",
                "tracestate":"bike=fixture"
            },
            "window":null,"revoke_remote":false
        })).unwrap();
        Self {
            event,
            activity_id: activity["id"].as_i64().unwrap(),
        }
    }
    pub fn activity(&self) -> ActivitySource {
        ActivitySource {
            event: self.event.clone(),
            activity: object("activity", include_bytes!("fixtures/activity.json")),
        }
    }
    pub fn sources(&self) -> ActivitySources {
        ActivitySources {
            event: self.event.clone(),
            activity: self.activity().activity,
            streams: object("streams", include_bytes!("fixtures/streams.json")),
        }
    }
    pub fn normalized(&self) -> NormalizedActivity {
        let activity: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/activity.json")).unwrap();
        let streams: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/streams.json")).unwrap();
        let source = serde_json::to_vec(&json!({"v":1,"provider":"strava","provider_activity_id":self.activity_id,"activity":activity,"streams":streams})).unwrap();
        NormalizedActivity {
            event: self.event.clone(),
            source: object("source", &source),
            parsed: Some(object("parsed", b"fixture parsed activity")),
        }
    }
    pub fn change(&self, status: ActivityStatus) -> ActivityChange {
        ActivityChange {
            event: self.event.clone(),
            status,
            bike_activity_id: Some(42),
            import_id: Some(18),
            segment_ids: vec![3],
            dirty_from: Some(chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()),
            removed_source: None,
        }
    }
    pub fn collection(&self) -> CollectionInput {
        CollectionInput {
            event: self.event.clone(),
            window: ActivityWindow {
                after: 1790769600,
                before: Some(1790942400),
            },
        }
    }
}
fn object(name: &str, bytes: &[u8]) -> ObjectRef {
    ObjectRef {
        path: format!("fixture:{name}"),
        bytes: bytes.len(),
        sha256: checksum(bytes),
    }
}
