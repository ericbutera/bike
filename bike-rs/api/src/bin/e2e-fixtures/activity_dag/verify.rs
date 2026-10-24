use super::{Input, Report, Result};
use bike_core::{
    activity_data::{deserialize_derived_activity_data, serialize_route_point_series, StoredActivityDerivedData},
    entities::{
        activities, activity_cards, activity_climbs, activity_earned_awards, activity_metadata, activity_recordings,
        activity_training_analyses, heatmap_projections, segment_efforts,
    },
    object_store::{ObjectOwner, ObjectStore},
};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

pub(super) async fn saved_outputs(db: &DatabaseConnection, input: &Input, report: &mut Report) -> Result<Value> {
    let id = input.reference["activity"]["id"]
        .as_i64()
        .ok_or("Reference activity missing")? as i32;
    let owner = input.reference["activity"]["user_id"]
        .as_i64()
        .ok_or("Reference owner missing")? as i32;
    let activity = activities::Model::find_owned(db, id, owner)
        .await?
        .ok_or("Processed activity missing")?;
    verify_activity(&activity, input, report)?;
    verify_ingestion_source(db, &activity, report).await?;
    let efforts = segment_efforts::Model::list_by_user_activity_ids(db, owner, &[id]).await?;
    let windows = effort_windows(&serde_json::to_value(&efforts)?);
    let expected = effort_windows(&input.reference["efforts"]);
    report.check(
        "every segment and ordered effort window matches frozen results",
        windows == expected,
    );
    let recording = verify_recording(db, input, report, id, owner).await?;
    verify_metadata(db, input, report, &activity).await?;
    let climbs = verify_climbs(db, input, report, id).await?;
    let training = activity_training_analyses::Model::for_activity(db, id).await?;
    report.check("saved training facts were produced", training.is_some());
    if let Some(training) = training {
        verify_training(&training, input, report);
    }
    verify_consumers(db, &activity, report).await?;
    let awards = activity_earned_awards::Model::for_user_activity_ids(db, owner, &[id]).await?;
    verify_awards(&awards, input, report);
    report.comparisons = json!({"efforts":windows,"expected_efforts":expected,"climbs":climbs});
    let identities: Vec<_> = efforts
        .iter()
        .map(|effort| {
            json!([
                effort.id,
                effort.segment_id,
                effort.effort_index,
                effort.user_rank,
                effort.overall_rank,
                effort.created_at
            ])
        })
        .collect();
    Ok(json!({"efforts":identities,"recording":recording,"awards":awards}))
}

fn verify_activity(activity: &activities::Model, input: &Input, report: &mut Report) -> Result<()> {
    let actual = serde_json::to_value(activity)?;
    for field in [
        "distance_meters",
        "moving_time_seconds",
        "total_time_seconds",
        "average_speed_mps",
        "max_speed_mps",
        "average_heart_rate_bpm",
        "max_heart_rate_bpm",
        "average_cadence_rpm",
        "max_cadence_rpm",
        "elevation_gain_meters",
        "elevation_loss_meters",
    ] {
        report.check(
            format!("activity {field}"),
            actual[field] == input.reference["activity"][field],
        );
    }
    Ok(())
}

async fn verify_metadata(
    db: &DatabaseConnection,
    input: &Input,
    report: &mut Report,
    activity: &activities::Model,
) -> Result<()> {
    let (id, owner) = (activity.id, activity.user_id);
    let metadata = activity_metadata::Model::load(db, id, owner).await?;
    report.check("typed metadata and ordered laps were produced", metadata.is_some());
    if let Some(details) = metadata {
        let expected: bike_core::activity_parser::strava::StravaActivitySummary =
            serde_json::from_value(input.provider_payload["activity"].clone())?;
        let expected = expected.source_metadata();
        let stored = &details.metadata;
        report.check(
            "provider description, location and private notes reach typed metadata",
            (
                stored.description.clone(),
                stored.location.clone(),
                stored.private_notes.clone(),
            ) == (expected.description, expected.location, expected.private_notes),
        );
        report.check(
            "provider gear reaches typed metadata",
            stored.gear_id == expected.gear.as_ref().map(|gear| gear.id.clone())
                && stored.gear_name == expected.gear.and_then(|gear| gear.name),
        );
        let stored: StoredActivityDerivedData =
            serde_json::from_value(input.reference["activity"]["derived_data_json"].clone())?;
        report.check(
            "lap facts retain original ordering and values",
            details.laps == deserialize_derived_activity_data(Some(&stored)).laps,
        );
    }
    Ok(())
}

async fn verify_climbs(db: &DatabaseConnection, input: &Input, report: &mut Report, id: i32) -> Result<Value> {
    let climbs = activity_climbs::Model::for_activity(db, id).await?;
    let climb_values: Vec<_> = climbs
        .iter()
        .map(|climb| {
            json!([
                climb.climb_number,
                climb.start_elapsed_seconds,
                climb.end_elapsed_seconds,
                climb.duration_seconds,
                climb.distance_meters,
                climb.elevation_gain_meters
            ])
        })
        .collect();
    let expected_climbs: Vec<_> = input.reference["climbs"]
        .as_array()
        .ok_or("Reference climbs missing")?
        .iter()
        .map(|climb| {
            json!([
                climb["climb_number"],
                climb["start_elapsed_seconds"],
                climb["end_elapsed_seconds"],
                climb["duration_seconds"],
                climb["distance_meters"].as_f64(),
                climb["elevation_gain_meters"].as_f64()
            ])
        })
        .collect();
    report.check(
        "climb producer ran and retained saved climb bounds and metrics",
        climb_values == expected_climbs,
    );
    Ok(json!({"actual":climb_values,"expected":expected_climbs}))
}

async fn verify_consumers(db: &DatabaseConnection, activity: &activities::Model, report: &mut Report) -> Result<()> {
    let (id, owner) = (activity.id, activity.user_id);
    report.check(
        "card producer ran",
        activity_cards::Entity::find_by_id(id).one(db).await?.is_some(),
    );
    let day = activity.started_at.date_naive();
    let fitness = bike_core::entities::fitness_freshness_daily::Model::between_days(db, owner, Some(day), day).await?;
    report.check(
        "fitness producer saves this rides positive daily training load",
        fitness
            .first()
            .is_some_and(|row| row.activity_count == 1 && row.training_load > 0.0),
    );
    let previews = activity_cards::preview_points::Model::for_owned_activities(db, owner, &[id]).await?;
    report.check(
        "card preview saves points from the recording",
        previews.get(&id).is_some_and(|points| !points.is_empty()),
    );
    let projection = heatmap_projections::Model::for_activity(db, id).await?;
    report.check(
        "heatmap was published",
        projection.is_some_and(|row| row.status == "ready"),
    );
    Ok(())
}
async fn verify_recording(
    db: &DatabaseConnection,
    input: &Input,
    report: &mut Report,
    id: i32,
    owner: i32,
) -> Result<Value> {
    let objects = ObjectStore::filesystem(
        &bike_core::config::Config::get().uploads_dir,
        ObjectOwner::ActivityImports,
    );
    let recording = activity_recordings::Model::load(db, &objects, id, owner).await?;
    report.check(
        "P1 recording exists and its artifact integrity passes",
        recording.is_some(),
    );
    if let Some(recording) = recording {
        let stored: bike_core::activity_data::StoredActivityDerivedData =
            serde_json::from_value(input.reference["activity"]["derived_data_json"].clone())?;
        let expected = deserialize_derived_activity_data(Some(&stored));
        report.check(
            "recording points and speed signals preserve existing encoding precision",
            serialize_route_point_series(&recording.route_points)
                == serialize_route_point_series(&expected.route_points),
        );
        report.check(
            "recording chart samples preserve sensor values and speed",
            chart_facts(recording.chart_points) == chart_facts(expected.chart_points),
        );
    }
    let reference = activity_recordings::Model::for_owned_activity(db, id, owner).await?;
    Ok(reference.map_or(
        Value::Null,
        |row| json!({"path":row.object_path,"sha256":row.object_sha256,"bytes":row.object_bytes}),
    ))
}

fn verify_training(training: &activity_training_analyses::Model, input: &Input, report: &mut Report) {
    let actual = json!({"ride_focus":training.ride_focus,"route_family_key":training.route_family_key,
        "z2_time_seconds":training.z2_time_seconds,"z2_distance_meters":training.z2_distance_meters,
        "z2_average_speed_mps":training.z2_average_speed_mps,"climbing_time_seconds":training.climbing_time_seconds,
        "climbing_elevation_gain_meters":training.climbing_elevation_gain_meters,"sustained_climb_count":training.sustained_climb_count});
    for field in [
        "ride_focus",
        "route_family_key",
        "z2_time_seconds",
        "z2_distance_meters",
        "z2_average_speed_mps",
        "climbing_time_seconds",
        "climbing_elevation_gain_meters",
        "sustained_climb_count",
    ] {
        report.check(
            format!("training {field}"),
            actual[field] == input.reference["training_analysis"][field],
        );
    }
}

fn effort_windows(rows: &Value) -> Vec<Value> {
    let mut windows: Vec<_> = rows
        .as_array()
        .into_iter()
        .flatten()
        .map(|effort| {
            json!([
                effort["segment_id"],
                effort["effort_index"],
                effort["start_route_point_index"],
                effort["end_route_point_index"],
                effort["start_elapsed_seconds"],
                effort["end_elapsed_seconds"],
                effort["duration_seconds"],
                effort["distance_meters"].as_f64()
            ])
        })
        .collect();
    windows.sort_by_key(Value::to_string);
    windows
}

fn verify_awards(awards: &[activity_earned_awards::Model], input: &Input, report: &mut Report) {
    let mut ordered = awards.to_vec();
    ordered.sort_by_key(|award| award.position);
    let actual: Vec<_> = ordered
        .iter()
        .map(|award| {
            json!({
                "segment_id":award.segment_id,"effort_index":award.effort_index,"segment_title":award.segment_title,
                "overall_rank":award.overall_rank,"personal_rank":award.personal_rank,
                "personal_best_duration_seconds":award.personal_best_duration_seconds,
            })
        })
        .collect();
    report.check(
        "five frozen legacy awards survive shared processing",
        json!(actual) == input.reference["analytics"]["achievement_highlights_json"]["items"],
    );
}

fn chart_facts(points: Vec<bike_core::activity_data::ActivityChartPoint>) -> Value {
    serde_json::to_value(bike_core::activity_data::serialize_derived_activity_data(
        &bike_core::activity_data::ActivityDerivedData {
            chart_points: points,
            ..Default::default()
        },
    ))
    .expect("Chart facts serialize")
}

async fn verify_ingestion_source(
    db: &DatabaseConnection,
    activity: &activities::Model,
    report: &mut Report,
) -> Result<()> {
    let import_id = activity
        .activity_import_id
        .ok_or("Processed activity lost its import")?;
    let import = bike_core::entities::activity_imports::Entity::find_owned(db, activity.user_id, import_id)
        .await?
        .ok_or("Processed import missing")?;
    report.check(
        "Strava ingestion source persists on the activity and import",
        activity.source == "strava_sync" && import.source == "strava_sync",
    );
    Ok(())
}
