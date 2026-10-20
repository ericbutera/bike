use super::recording::recording;
use bike_core::auth::entities::{refresh_tokens, users};
use bike_core::entities::{activities, segment_efforts, segments};
use bike_core::test_support as fixtures;
use chrono::Duration;
use serde_json::{json, Value};
use uuid::Uuid;

pub const OWNERS: i32 = 10;
pub const ACTIVITIES: i32 = 100;
pub const STORAGE_BUDGET: i64 = 128 * 1024 * 1024;
pub const SESSION: &str = "perf08-small-owner-1";

pub fn manifest() -> Value {
    let (fingerprint, input_bytes) = fingerprint();
    assert!(input_bytes < STORAGE_BUDGET as usize, "fixture exceeds storage budget");
    json!({"version": 2, "seed": 8, "input_sha256": fingerprint, "input_bytes": input_bytes, "logical_clock": fixtures::timestamp(),
        "owners": OWNERS, "activities_per_owner": ACTIVITIES, "activities": OWNERS * ACTIVITIES,
        "segments": OWNERS, "efforts": OWNERS * (ACTIVITIES - 1) * 2,
        "route_points_per_activity": 201, "chart_points_per_activity": 101,
        "storage_budget_bytes": STORAGE_BUDGET,
        "empty_activity": 1001, "missing_activity": 1002, "stale_activity": 1003,
        "ready_activity": 1100, "segment": 1,
        "note": "synthetic short recordings; actual worker analytics builders; no worker throughput claim"})
}

pub fn owner(id: i32) -> users::Model {
    users::Model {
        id,
        pid: Uuid::from_u128(id as u128),
        email: format!("perf08-{id}@example.test"),
        name: format!("Performance owner {id}"),
        api_key: format!("perf08-owner-{id}"),
        ..fixtures::customer()
    }
}

pub fn session() -> refresh_tokens::Model {
    refresh_tokens::Model {
        token: SESSION.into(),
        user_pid: owner(1).pid,
        expires_at: 4_102_444_800,
        ..fixtures::refresh_token()
    }
}

pub fn activity(owner: i32, index: i32) -> activities::Model {
    let step = 3 + index % 5;
    let at = fixtures::timestamp() - Duration::days(i64::from(ACTIVITIES - index));
    activities::Model {
        id: owner * 1000 + index,
        user_id: owner,
        title: format!("Owner {owner} ride {index}"),
        started_at: at,
        ended_at: Some(at + Duration::seconds(i64::from(step * 200))),
        distance_meters: Some(2223.9),
        moving_time_seconds: Some(step * 200),
        total_time_seconds: Some(step * 200),
        elevation_gain_meters: Some(100.0),
        average_speed_mps: Some(11.1195 / f64::from(step)),
        derived_data_json: Some((&recording(step)).into()),
        created_at: at,
        updated_at: at,
        ..fixtures::activity()
    }
}

pub fn segment(owner: i32) -> segments::Model {
    let route = recording(3).route_points[..=50].to_vec();
    segments::Model {
        id: owner,
        user_id: owner,
        title: format!("Owner {owner} repeated climb"),
        route_data_json: Some(bike_core::activity_details::StoredRoutePointSeries(route)),
        distance_meters: Some(555.975),
        ..fixtures::segment()
    }
}

pub fn effort(owner: i32, index: i32, occurrence: i32) -> segment_efforts::Model {
    let start = (occurrence - 1) * 100;
    let step = 3 + index % 5;
    segment_efforts::Model {
        id: owner * 10000 + index * 2 + occurrence,
        user_id: owner,
        segment_id: owner,
        activity_id: owner * 1000 + index,
        effort_index: occurrence,
        start_route_point_index: start,
        end_route_point_index: start + 50,
        start_elapsed_seconds: start * step,
        end_elapsed_seconds: (start + 50) * step,
        duration_seconds: 50 * step,
        distance_meters: Some(555.975),
        ..fixtures::segment_effort()
    }
}

pub fn fingerprint() -> (String, usize) {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    let mut bytes = 0;
    for id in 1..=OWNERS {
        for row in [
            serde_json::to_value(owner(id)).unwrap(),
            serde_json::to_value(segment(id)).unwrap(),
        ] {
            let encoded = serde_json::to_vec(&row).unwrap();
            bytes += encoded.len();
            hash.update(encoded);
        }
        for index in 1..=ACTIVITIES {
            let encoded = serde_json::to_vec(&activity(id, index)).unwrap();
            bytes += encoded.len();
            hash.update(encoded);
            if index == 1 {
                continue;
            }
            for occurrence in 1..=2 {
                let encoded = serde_json::to_vec(&effort(id, index, occurrence)).unwrap();
                bytes += encoded.len();
                hash.update(encoded);
            }
        }
    }
    (hex::encode(hash.finalize()), bytes)
}
