pub fn integration_event() -> crate::entities::integration_events::Model {
    crate::entities::integration_events::Model {
        id: 31,
        user_id: Some(super::customer().id),
        provider: "strava".into(),
        event_type: "strava.event.accepted".into(),
        level: "info".into(),
        message: "Sync requested".into(),
        connection_id: Some(super::connection().id),
        payload: Some(serde_json::json!({
            "operation": "sync", "temporal_workflow_id": "strava-fixture"
        })),
        created_at: super::timestamp(),
    }
}

pub fn quota_bucket() -> crate::entities::provider_rate_limit_buckets::Model {
    crate::entities::provider_rate_limit_buckets::Model {
        id: 1,
        provider: "fixture-quota".into(),
        bucket: "current".into(),
        limit_count: 200,
        used_count: 17,
        reset_at: super::timestamp(),
        created_at: super::timestamp(),
        updated_at: super::timestamp(),
    }
}
