//! Connected matching acceptance, invoked only in the disposable E2E database.
use bike_core::{
    activity_data::{ActivityDerivedData, ActivityRoutePoint},
    activity_parser::parse_activity_data,
    entities::{activities, route_matching_bounds as bounds, segment_efforts, segments},
    segment_support::{replace_segment_efforts_for_activity, serialize_segment_route_points},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type Pages = Vec<Vec<i32>>;
type EffortWindows = BTreeMap<String, Vec<[i32; 5]>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    user_id: i32,
    activities: Vec<Recording>,
    segments: Vec<SourceFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recording {
    user_id: i32,
    file: SourceFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFile {
    filename: String,
    contents_hex: String,
}

impl SourceFile {
    fn route(&self) -> Result<Vec<ActivityRoutePoint>> {
        let format = self.filename.rsplit('.').next().ok_or("Fixture has no extension")?;
        Ok(
            parse_activity_data(&self.filename, format, &hex::decode(&self.contents_hex)?)?
                .derived_data
                .route_points,
        )
    }
}

#[derive(Serialize)]
pub(super) struct Report {
    activities: Vec<ActivityReport>,
    segments: Vec<SegmentReport>,
    controls: Vec<QueryCase>,
}

#[derive(Serialize)]
struct ActivityReport {
    filename: String,
    id: i32,
    segment_pages: Pages,
    efforts: EffortWindows,
}

#[derive(Serialize)]
struct SegmentReport {
    filename: String,
    id: i32,
    activity_pages: Pages,
}

#[derive(Serialize)]
struct QueryCase {
    name: &'static str,
    pages: Pages,
    expected: Vec<i32>,
}

pub(super) async fn run(db: &DatabaseConnection, input: Input) -> Result<Report> {
    let mut definitions = BTreeMap::new();
    for file in input.segments {
        let route = file.route()?;
        let segment = insert_segment(db, input.user_id, &file.filename, &route).await?;
        bounds::segment::Model::store_route(db, segment.id, &route).await?;
        definitions.insert(segment.id, file.filename);
    }
    let first_segment = *definitions.first_key_value().ok_or("No segment fixtures")?.0;
    let mut recordings = Vec::new();
    for recording in input.activities {
        let route = recording.file.route()?;
        let activity = insert_activity(db, recording.user_id, &recording.file.filename, "ride", &route).await?;
        bounds::Model::store_source(db, &activity).await?;
        recordings.push((recording.file.filename, activity, route));
    }
    let first_activity = recordings.first().ok_or("No activity fixtures")?.1.id;
    let mut activities = Vec::new();
    for (filename, activity, route) in &recordings {
        let segment_pages = segment_pages(db, bounds::Bounds::from_route(route)?, first_segment - 1).await?;
        replace_segment_efforts_for_activity(db, activity.user_id, activity.id, route).await?;
        activities.push(ActivityReport {
            filename: filename.clone(),
            id: activity.id,
            segment_pages,
            efforts: saved_windows(db, activity.id, &definitions).await?,
        });
    }
    let mut segments = Vec::new();
    for (&id, filename) in &definitions {
        let segment = segments::Model::find_by_id(db, id)
            .await?
            .ok_or("Fixture segment disappeared")?;
        let route = bike_core::segment_support::deserialize_segment_route_points(segment.route_data_json.as_ref());
        segments.push(SegmentReport {
            filename: filename.clone(),
            id,
            activity_pages: activity_pages(db, bounds::Bounds::from_route(&route)?, first_activity - 1).await?,
        });
    }
    let controls = controls(db, input.user_id, &recordings[0].2).await?;
    Ok(Report {
        activities,
        segments,
        controls,
    })
}

async fn insert_activity(
    db: &DatabaseConnection,
    user_id: i32,
    title: &str,
    sport: &str,
    route: &[ActivityRoutePoint],
) -> Result<activities::Model> {
    let derived = ActivityDerivedData {
        route_points: route.to_vec(),
        ..Default::default()
    };
    Ok(activities::ActiveModel {
        user_id: Set(user_id),
        title: Set(title.into()),
        sport: Set(sport.into()),
        source: Set("manual_upload".into()),
        started_at: Set(Utc::now()),
        derived_data_json: Set(Some((&derived).into())),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

async fn insert_segment(
    db: &DatabaseConnection,
    user_id: i32,
    title: &str,
    route: &[ActivityRoutePoint],
) -> Result<segments::Model> {
    Ok(segments::ActiveModel {
        user_id: Set(user_id),
        title: Set(title.into()),
        source: Set("manual_segment_import".into()),
        route_data_json: Set(Some(serialize_segment_route_points(route))),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

async fn saved_windows(
    db: &DatabaseConnection,
    activity_id: i32,
    definitions: &BTreeMap<i32, String>,
) -> Result<EffortWindows> {
    let efforts = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::ActivityId.eq(activity_id))
        .all(db)
        .await?;
    let mut result = BTreeMap::new();
    for (&id, filename) in definitions {
        let mut rows: Vec<_> = efforts.iter().filter(|effort| effort.segment_id == id).collect();
        rows.sort_by_key(|effort| effort.effort_index);
        result.insert(
            filename.clone(),
            rows.into_iter()
                .map(|effort| {
                    [
                        effort.start_route_point_index,
                        effort.end_route_point_index,
                        effort.start_elapsed_seconds,
                        effort.end_elapsed_seconds,
                        effort.duration_seconds,
                    ]
                })
                .collect(),
        );
    }
    Ok(result)
}

async fn activity_pages(db: &DatabaseConnection, bounds: Option<bounds::Bounds>, mut after: i32) -> Result<Pages> {
    let mut pages = Vec::new();
    loop {
        let page = bounds::Model::matching_activity_ids(db, bounds, after, 1).await?;
        let Some(&last) = page.last() else { break };
        if last <= after {
            return Err("Activity candidate cursor did not advance".into());
        }
        after = last;
        pages.push(page);
    }
    Ok(pages)
}

async fn segment_pages(db: &DatabaseConnection, bounds: Option<bounds::Bounds>, mut after: i32) -> Result<Pages> {
    let mut pages = Vec::new();
    loop {
        let page = segments::Model::matching_routes_page(db, after, 3, bounds).await?;
        let Some(last) = page.last() else { break };
        if last.id <= after {
            return Err("Segment candidate cursor did not advance".into());
        }
        after = last.id;
        pages.push(page.into_iter().map(|segment| segment.id).collect());
    }
    Ok(pages)
}

async fn controls(db: &DatabaseConnection, user_id: i32, template: &[ActivityRoutePoint]) -> Result<Vec<QueryCase>> {
    let mut route = template[..2].to_vec();
    route[0].latitude = 0.0;
    route[0].longitude = 0.0;
    route[1].latitude = 0.0;
    route[1].longitude = 0.01;
    let source = bounds::Bounds::from_route(&route)?.ok_or("Control route missing bounds")?;
    let mut touching = route.clone();
    for point in &mut touching {
        point.latitude = source.matching_corridor().max_latitude;
    }
    let mut distant = route.clone();
    for point in &mut distant {
        point.latitude = source.matching_corridor().max_latitude + 0.01;
    }
    let mut cases = activity_controls(db, user_id, &route, &touching, &distant).await?;
    cases.extend(segment_controls(db, user_id, &route, &touching, &distant).await?);
    Ok(cases)
}

async fn activity_controls(
    db: &DatabaseConnection,
    user_id: i32,
    route: &[ActivityRoutePoint],
    touching: &[ActivityRoutePoint],
    distant: &[ActivityRoutePoint],
) -> Result<Vec<QueryCase>> {
    let inside = insert_activity(db, user_id, "inside", "ride", route).await?;
    let touch = insert_activity(db, user_id, "touching", "ride", touching).await?;
    let far = insert_activity(db, user_id, "distant", "ride", distant).await?;
    let missing = insert_activity(db, user_id, "missing bounds", "ride", distant).await?;
    let empty = insert_activity(db, user_id, "empty", "ride", &[]).await?;
    let single = insert_activity(db, user_id, "one point", "ride", &route[..1]).await?;
    let run = insert_activity(db, user_id, "noncycling", "run", route).await?;
    for activity in [&inside, &touch, &far, &empty, &single, &run] {
        bounds::Model::store_source(db, activity).await?;
    }
    // Put the boundary box at its exact f64 coordinate. Compact GPS recording
    // encoding otherwise rounds the latitude before store_source reads it back.
    bounds::ActiveModel {
        activity_id: Set(touch.id),
        min_latitude: Set(Some(touching[0].latitude)),
        max_latitude: Set(Some(touching[0].latitude)),
        ..Default::default()
    }
    .update(db)
    .await?;
    Ok(vec![
        QueryCase {
            name: "activity overlap, inclusive boundary, missing bounds and cycling eligibility",
            pages: activity_pages(db, bounds::Bounds::from_route(route)?, inside.id - 1).await?,
            expected: vec![inside.id, touch.id, missing.id],
        },
        QueryCase {
            name: "activity search without spatial bounds",
            pages: activity_pages(db, None, inside.id - 1).await?,
            expected: vec![inside.id, touch.id, far.id, missing.id, empty.id, single.id],
        },
    ])
}

async fn segment_controls(
    db: &DatabaseConnection,
    user_id: i32,
    route: &[ActivityRoutePoint],
    touching: &[ActivityRoutePoint],
    distant: &[ActivityRoutePoint],
) -> Result<Vec<QueryCase>> {
    let inside = insert_segment(db, user_id, "inside", route).await?;
    let touch = insert_segment(db, user_id, "touching", touching).await?;
    let far = insert_segment(db, user_id, "distant", distant).await?;
    let missing = insert_segment(db, user_id, "missing bounds", distant).await?;
    let empty = insert_segment(db, user_id, "empty", &[]).await?;
    let single = insert_segment(db, user_id, "one point", &route[..1]).await?;
    for (segment, points) in [
        (&inside, route),
        (&touch, touching),
        (&far, distant),
        (&empty, &[]),
        (&single, &route[..1]),
    ] {
        bounds::segment::Model::store_route(db, segment.id, points).await?;
    }
    Ok(vec![
        QueryCase {
            name: "segment overlap, inclusive boundary, missing bounds and unmatchable routes",
            pages: segment_pages(db, bounds::Bounds::from_route(route)?, inside.id - 1).await?,
            expected: vec![inside.id, touch.id, missing.id],
        },
        QueryCase {
            name: "segment search without spatial bounds",
            pages: segment_pages(db, None, inside.id - 1).await?,
            expected: vec![inside.id, touch.id, far.id, missing.id, empty.id, single.id],
        },
    ])
}
