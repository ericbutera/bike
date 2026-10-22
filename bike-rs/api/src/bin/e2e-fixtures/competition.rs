use super::{recording::Routes, store_record, Operation, Record};
use bike_core::activity_data::serialize_derived_activity_data;
use bike_core::activity_parser::{parse_activity_data, ParsedActivityData};
use bike_core::analytics::rebuild_segment_analytics_cache;
use bike_core::entities::segments;
use bike_core::object_store::{ObjectOwner, ObjectRef, ObjectStore};
use bike_core::segment_support::{replace_segment_efforts_for_activity, serialize_segment_route_points};
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, TransactionTrait};
use serde::Deserialize;
use serde_json::json;

pub type FixtureResult<T> = Result<T, Box<dyn std::error::Error>>;
const FIRST_SEGMENT_ID: i32 = 4101;
const FIRST_SEGMENT_TITLE: &str = "Fixture City Loop - Breakin The Law";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Competition {
    users: Vec<Record>,
    pub rides: Vec<Ride>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ride {
    pub id: i32,
    pub user_id: i32,
    pub route: String,
    pub started_at: DateTime<Utc>,
    pub pace: f64,
    pub repeats: i32,
}

pub async fn seed(db: &DatabaseConnection, competition: Competition) -> FixtureResult<()> {
    let tx = db.begin().await?;
    if let Some(segment) = segments::Entity::find_by_id(FIRST_SEGMENT_ID).one(&tx).await? {
        if segment.title != FIRST_SEGMENT_TITLE {
            return Err("Competition fixture segment ID belongs to different data".into());
        }
        println!("Existing segment competition fixtures preserved");
        return Ok(());
    }
    let uploads = std::env::var("UPLOADS_DIR")?;
    let objects = ObjectStore::filesystem(uploads, ObjectOwner::ActivityImports);
    let routes = Routes::load()?;
    for user in competition.users {
        store_record(&tx, user).await?;
    }
    store_segments(&tx).await?;
    // Baseline fixtures use explicit IDs. These normal processing functions
    // allocate IDs, so advance their sequences before invoking them.
    for table in ["segment_efforts", "segment_user_summaries"] {
        tx.execute_unprepared(&format!(
            "SELECT setval(pg_get_serial_sequence('{table}', 'id'), COALESCE((SELECT max(id) FROM {table}), 1), EXISTS (SELECT 1 FROM {table}))"
        )).await?;
    }
    for ride in competition.rides {
        let bytes = routes.gpx(&ride)?;
        let parsed = parse_activity_data("fixture.gpx", "gpx", &bytes)?;
        let key = format!("activity-imports/segment-competition/{}.gpx", ride.id);
        let object = objects.write_key(&key, &bytes).await?;
        store_ride(&tx, &ride, &parsed, &object).await?;
        replace_segment_efforts_for_activity(&tx, ride.user_id, ride.id, &parsed.derived_data.route_points).await?;
    }
    rebuild_segment_analytics_cache(&tx, &[4101, 4102, 4103, 4104, 4105]).await?;
    tx.commit().await?;
    println!("Segment competition fixtures created");
    Ok(())
}

async fn insert(tx: &DatabaseTransaction, entity: &str, values: serde_json::Value) -> FixtureResult<()> {
    store_record(
        tx,
        Record {
            entity: entity.into(),
            operation: Operation::Insert,
            values,
        },
    )
    .await?;
    Ok(())
}

async fn store_segments(tx: &DatabaseTransaction) -> FixtureResult<()> {
    let sources: [(&str, &[u8]); 5] = [
        (
            FIRST_SEGMENT_TITLE,
            include_bytes!("../../../tests/fixtures/segment_matching/segments/Segment - Breakin The Law.gpx"),
        ),
        (
            "Fixture City Loop - Country Club Boys",
            include_bytes!("../../../tests/fixtures/segment_matching/segments/Country Club Boys.gpx"),
        ),
        (
            "Fixture City Loop - East Side Flow",
            include_bytes!("../../../tests/fixtures/segment_matching/segments/East Side Flow.gpx"),
        ),
        (
            "Fixture Unmarked - Jam Sesh",
            include_bytes!("../../../tests/fixtures/segment_matching/segments/Segment - Jam Sesh.gpx"),
        ),
        (
            "Fixture Unmarked - The Holy Grail",
            include_bytes!(
                "../../../tests/fixtures/segment_matching/segments/Segment - The Holy Grail Single Track.gpx"
            ),
        ),
    ];
    for (offset, (title, bytes)) in sources.into_iter().enumerate() {
        let parsed = parse_activity_data("fixture-segment.gpx", "gpx", bytes)?;
        insert(
            tx,
            "segments",
            json!({
                "id": FIRST_SEGMENT_ID + offset as i32, "user_id": 1,
                "title": title, "source": "manual_upload", "format": "gpx",
                "original_filename": format!("{title}.gpx"), "mode": "xc", "starred": true,
                "distance_meters": parsed.draft.distance_meters,
                "route_data_json": serialize_segment_route_points(&parsed.derived_data.route_points),
                "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z",
                "last_activity_change_at": "2026-07-06T08:00:00Z"
            }),
        )
        .await?;
    }
    Ok(())
}

async fn store_ride(
    tx: &DatabaseTransaction,
    ride: &Ride,
    parsed: &ParsedActivityData,
    object: &ObjectRef,
) -> FixtureResult<()> {
    let filename = format!("fixture-competition-{}.gpx", ride.id);
    let finished = parsed.draft.ended_at.unwrap_or(ride.started_at);
    insert(
        tx,
        "activity_imports",
        json!({
            "id": ride.id, "user_id": ride.user_id, "source": "manual_upload", "format": "gpx",
            "status": "processed", "original_filename": filename, "storage_path": object.path,
            "size_bytes": object.bytes, "mime_type": "application/gpx+xml", "activity_id": null,
            "processing_stage": "complete", "processing_attempts": 1, "processed_at": finished,
            "created_at": ride.started_at, "updated_at": finished
        }),
    )
    .await?;
    insert(
        tx,
        "activity_import_artifacts",
        json!({
            "id": ride.id, "activity_import_id": ride.id, "user_id": ride.user_id,
            "artifact_kind": "original", "format": "gpx", "source_quality": "gpx_original",
            "original_filename": filename, "storage_path": object.path, "size_bytes": object.bytes,
            "mime_type": "application/gpx+xml", "checksum_sha256": object.sha256, "created_at": ride.started_at
        }),
    )
    .await?;
    let mut values = serde_json::to_value(&parsed.draft)?;
    let fields = values.as_object_mut().ok_or("Activity draft is not an object")?;
    let extra = json!({
        "id": ride.id, "user_id": ride.user_id, "activity_import_id": ride.id,
        "source": "manual_upload", "format": "gpx", "original_filename": filename,
        "activity_type": "training", "derived_data_json": serialize_derived_activity_data(&parsed.derived_data),
        "created_at": ride.started_at, "updated_at": finished
    });
    fields.extend(extra.as_object().ok_or("Fixture fields are not an object")?.clone());
    insert(tx, "activities", values).await?;
    store_record(
        tx,
        Record {
            entity: "activity_imports".into(),
            operation: Operation::Update,
            values: json!({"id": ride.id, "activity_id": ride.id}),
        },
    )
    .await?;
    Ok(())
}
