use super::*;
use crate::activity_recording::RecordingEnvironment;

#[test]
fn native_json_retains_unmodeled_summary_fields_and_virtual_recorder_evidence() {
    let mut input: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../testdata/strava-real-ride.json")).unwrap();
    input["activity"]["device_name"] = "Zwift".into();
    input["activity"]["external_id"] = "zwift-activity.fit".into();
    input["activity"]["future_provider_field"] = serde_json::json!({"nested":"retained"});
    let payload: StoredStravaProviderPayload = serde_json::from_value(input).unwrap();
    let stored = crate::strava::build_activity_upload(&payload.activity, &payload.streams).unwrap();
    assert_eq!(stored.format, "json");
    let retained: serde_json::Value = serde_json::from_slice(&stored.bytes).unwrap();
    assert_eq!(
        retained["activity"]["future_provider_field"]["nested"],
        "retained"
    );
    assert_eq!(retained["activity"]["device_name"], "Zwift");
    assert_eq!(
        parse_strava_provider_payload(&stored.bytes)
            .unwrap()
            .derived_data
            .recording
            .environment,
        RecordingEnvironment::Virtual
    );
}

#[test]
fn gateway_payload_preserves_each_independent_virtual_signal() {
    for (sport, legacy, trainer, name, expected) in [
        (
            "VirtualRide",
            "Ride",
            false,
            "Morning",
            RecordingEnvironment::Virtual,
        ),
        (
            "Ride",
            "VirtualRide",
            false,
            "Morning",
            RecordingEnvironment::Virtual,
        ),
        (
            "Ride",
            "Ride",
            true,
            "Morning",
            RecordingEnvironment::Indoor,
        ),
        (
            "Ride",
            "Ride",
            false,
            "Zwift workout",
            RecordingEnvironment::Virtual,
        ),
        (
            "Ride",
            "Ride",
            false,
            "Morning",
            RecordingEnvironment::Unknown,
        ),
        (
            "MountainBikeRide",
            "Ride",
            false,
            "Trail",
            RecordingEnvironment::Unknown,
        ),
    ] {
        let mut activity = test_activity_with_sport(Some(sport), Some(legacy));
        activity.trainer = Some(trainer);
        activity.name = name.into();
        let payload = StoredStravaProviderPayload::new(activity.clone(), test_streams());
        let parsed = parse_strava_provider_payload(&serde_json::to_vec(&payload).unwrap()).unwrap();
        assert_eq!(parsed.derived_data.recording.environment, expected);
    }
}
