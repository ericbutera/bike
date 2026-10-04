use crate::{
    activity_data::{self, ActivityRoutePoint, StoredActivityDerivedData, StoredRoutePointSeries},
    baseline,
    database::{Access, Raw},
    geometry::{self, Coordinates},
};
use anyhow::{ensure, Result};
use postgres::{Client, Row};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, time::Instant};

#[derive(Deserialize)]
pub struct Case {
    pub name: String,
    pub kind: String,
    pub id: i32,
    pub bbox: Option<[f64; 4]>,
    pub tile_bbox: Option<[f64; 4]>,
    pub warmups: usize,
    pub samples: usize,
    pub radius: f64,
    pub size: usize,
    pub gutter: usize,
    pub from: String,
}

#[derive(Default)]
pub struct Outcome {
    pub access: Access,
    pub decode_ms: f64,
    pub domain_ms: f64,
    pub output_ms: f64,
    pub candidate_rows: usize,
    pub vertices: usize,
    pub fingerprint: String,
    pub output_bytes: usize,
}

pub fn activity(row: &Row) -> Result<Vec<ActivityRoutePoint>> {
    let raw = row.get::<_, Option<Raw<'_>>>("derived_data_json");
    let stored = raw
        .map(|raw| serde_json::from_slice::<StoredActivityDerivedData>(raw.0))
        .transpose()?;
    Ok(activity_data::deserialize_derived_activity_data(stored.as_ref()).route_points)
}
pub fn segment(row: &Row) -> Result<Vec<ActivityRoutePoint>> {
    let raw = row.get::<_, Option<Raw<'_>>>("route_data_json");
    let stored = raw
        .map(|raw| serde_json::from_slice::<StoredRoutePointSeries>(raw.0))
        .transpose()?;
    Ok(activity_data::deserialize_route_point_series(
        stored.as_ref(),
    ))
}
pub fn coords(points: &[ActivityRoutePoint]) -> Coordinates {
    points.iter().map(|p| [p.longitude, p.latitude]).collect()
}

fn bounds(points: &[ActivityRoutePoint], radius: f64) -> [f64; 4] {
    let minx = points
        .iter()
        .map(|p| p.longitude)
        .fold(f64::INFINITY, f64::min);
    let maxx = points
        .iter()
        .map(|p| p.longitude)
        .fold(f64::NEG_INFINITY, f64::max);
    let miny = points
        .iter()
        .map(|p| p.latitude)
        .fold(f64::INFINITY, f64::min);
    let maxy = points
        .iter()
        .map(|p| p.latitude)
        .fold(f64::NEG_INFINITY, f64::max);
    let dy = radius / 110_000.0;
    let dx = dy
        / (miny.abs().max(maxy.abs()) + dy)
            .to_radians()
            .cos()
            .max(0.000001);
    [minx - dx, miny - dy, maxx + dx, maxy + dy]
}

fn matching(db: &mut Client, arm: &str, case: &Case, out: &mut Outcome) -> Result<Value> {
    let regeneration = case.kind == "segment_history";
    let source = if regeneration {
        out.access.query(
            db,
            "SELECT route_data_json FROM postgis_eval.segments WHERE id=$1",
            &[&case.id],
        )?
    } else {
        out.access.query(
            db,
            "SELECT derived_data_json FROM postgis_eval.activities WHERE user_id=1 AND id=$1",
            &[&case.id],
        )?
    };
    ensure!(source.len() == 1, "Missing matching input");
    let started = Instant::now();
    let input = if regeneration {
        segment(&source[0])?
    } else {
        activity(&source[0])?
    };
    out.decode_ms += started.elapsed().as_secs_f64() * 1000.0;
    ensure!(input.len() >= 2, "Matching case needs a route");
    let bbox = bounds(&input, case.radius);
    // Wide/polar/antimeridian bounding boxes must retain all candidates. Actual
    // Rust matching is still the oracle after the conservative prefilter.
    let conservative = bbox[2] - bbox[0] > 180.0
        || bbox[1] < -85.0
        || bbox[3] > 85.0
        || bbox[0] < -180.0
        || bbox[2] > 180.0;
    let mut after = 0;
    let mut matches = Vec::new();
    out.vertices += input.len();
    loop {
        let rows = if arm.ends_with('0') || conservative {
            if regeneration {
                out.access.query(db,"SELECT id,derived_data_json FROM postgis_eval.activities WHERE user_id=1 AND sport=ANY($1) AND id>$2 ORDER BY id LIMIT 16",&[&baseline::BIKE_ACTIVITY_SPORT_VALUES,&after])?
            } else {
                out.access.query(db,"SELECT id,route_data_json FROM postgis_eval.segments WHERE id>$1 ORDER BY id LIMIT 25",&[&after])?
            }
        } else if arm == "V1" {
            if regeneration {
                let first = &input[0];
                let last = input.last().unwrap();
                let firstbox = bounds(std::slice::from_ref(first), case.radius);
                let lastbox = bounds(std::slice::from_ref(last), case.radius);
                out.access.query(db,"SELECT a.id,a.derived_data_json FROM postgis_eval.activities a JOIN postgis_eval.activity_projection p USING(id) WHERE a.user_id=1 AND a.sport=ANY($1) AND p.maxx>=$2 AND p.minx<=$3 AND p.maxy>=$4 AND p.miny<=$5 AND p.maxx>=$6 AND p.minx<=$7 AND p.maxy>=$8 AND p.miny<=$9 AND a.id>$10 ORDER BY a.id LIMIT 16",&[&baseline::BIKE_ACTIVITY_SPORT_VALUES,&firstbox[0],&firstbox[2],&firstbox[1],&firstbox[3],&lastbox[0],&lastbox[2],&lastbox[1],&lastbox[3],&after])?
            } else {
                out.access.query(db,"SELECT s.id,s.route_data_json FROM postgis_eval.segments s JOIN postgis_eval.segment_projection p USING(id) WHERE p.startx BETWEEN $1 AND $2 AND p.starty BETWEEN $3 AND $4 AND p.endx BETWEEN $1 AND $2 AND p.endy BETWEEN $3 AND $4 AND s.id>$5 ORDER BY s.id LIMIT 25",&[&bbox[0],&bbox[2],&bbox[1],&bbox[3],&after])?
            }
        } else if regeneration {
            let first = &input[0];
            let last = input.last().unwrap();
            out.access.query(db,"SELECT a.id,a.derived_data_json FROM postgis_eval.activities a JOIN postgis_eval.activity_projection p USING(id) WHERE a.user_id=1 AND a.sport=ANY($1) AND ST_DWithin(p.geom::geography,ST_SetSRID(ST_MakePoint($2,$3),4326)::geography,$6,false) AND ST_DWithin(p.geom::geography,ST_SetSRID(ST_MakePoint($4,$5),4326)::geography,$6,false) AND a.id>$7 ORDER BY a.id LIMIT 16",&[&baseline::BIKE_ACTIVITY_SPORT_VALUES,&first.longitude,&first.latitude,&last.longitude,&last.latitude,&case.radius,&after])?
        } else {
            let line = geometry::wkb(&coords(&input));
            out.access.query(db,"SELECT s.id,s.route_data_json FROM postgis_eval.segments s JOIN postgis_eval.segment_projection p USING(id) WHERE ST_DWithin(ST_StartPoint(p.geom)::geography,ST_GeomFromWKB($1,4326)::geography,$2,false) AND ST_DWithin(ST_EndPoint(p.geom)::geography,ST_GeomFromWKB($1,4326)::geography,$2,false) AND s.id>$3 ORDER BY s.id LIMIT 25",&[&line,&case.radius,&after])?
        };
        if rows.is_empty() {
            break;
        }
        after = rows.last().unwrap().get("id");
        out.candidate_rows += rows.len();
        for row in rows {
            let started = Instant::now();
            let route = if regeneration {
                activity(&row)?
            } else {
                segment(&row)?
            };
            out.decode_ms += started.elapsed().as_secs_f64() * 1000.0;
            out.vertices += route.len();
            let started = Instant::now();
            let found = if regeneration {
                baseline::run_match(&input, &route)
            } else {
                baseline::run_match(&route, &input)
            };
            if !found.is_empty() {
                matches.push(json!([row.get::<_, i32>("id"), found]));
            }
            out.domain_ms += started.elapsed().as_secs_f64() * 1000.0;
        }
    }
    Ok(json!(matches))
}

fn race(db: &mut Client, case: &Case, out: &mut Outcome) -> Result<Value> {
    let segments = out.access.query(
        db,
        "SELECT route_data_json FROM postgis_eval.segments WHERE user_id=1 AND id=$1",
        &[&case.id],
    )?;
    ensure!(segments.len() == 1, "Missing owned segment");
    let efforts=out.access.query(db,"SELECT id,activity_id,user_id,start_route_point_index,end_route_point_index,duration_seconds FROM postgis_eval.segment_efforts WHERE segment_id=$1 ORDER BY duration_seconds,id",&[&case.id])?;
    let ids: Vec<i32> = efforts.iter().map(|r| r.get("activity_id")).collect();
    let users: Vec<i32> = efforts.iter().map(|r| r.get("user_id")).collect();
    let activities=out.access.query(db,"SELECT id,title,started_at,derived_data_json FROM postgis_eval.activities WHERE id=ANY($1)",&[&ids])?;
    let riders = out.access.query(
        db,
        "SELECT id,name FROM postgis_eval.users WHERE id=ANY($1)",
        &[&users],
    )?;
    let started = Instant::now();
    let mut stored = HashMap::new();
    for row in &activities {
        let data = row
            .get::<_, Option<Raw<'_>>>("derived_data_json")
            .map(|r| serde_json::from_slice::<StoredActivityDerivedData>(r.0))
            .transpose()?;
        stored.insert(row.get::<_, i32>("id"), data);
    }
    let reference = segment(&segments[0])?;
    out.decode_ms += started.elapsed().as_secs_f64() * 1000.0;
    out.candidate_rows = activities.len();
    let started = Instant::now();
    let mut responses = Vec::new();
    for effort in efforts {
        let id: i32 = effort.get("activity_id");
        let user: i32 = effort.get("user_id");
        if let Some(data) = stored.get(&id) {
            if !riders.iter().any(|r| r.get::<_, i32>("id") == user) {
                continue;
            }
            let points =
                activity_data::deserialize_derived_activity_data(data.as_ref()).route_points;
            out.vertices += points.len();
            let slice = baseline::slice_effort_route_points(
                &points,
                effort.get("start_route_point_index"),
                effort.get("end_route_point_index"),
            );
            responses.push(json!([
                effort.get::<_, i32>("id"),
                id,
                user,
                effort.get::<_, i32>("duration_seconds"),
                slice
            ]));
        }
    }
    out.domain_ms += started.elapsed().as_secs_f64() * 1000.0;
    Ok(json!({"route":reference,"efforts":responses}))
}

fn projected(
    db: &mut Client,
    arm: &str,
    case: &Case,
    out: &mut Outcome,
) -> Result<Vec<Coordinates>> {
    let heatmap = case.kind == "heatmap";
    if heatmap && arm.ends_with('0') {
        let mut routes = Vec::new();
        let mut after = 0;
        loop {
            let rows=out.access.query(db,"SELECT id,derived_data_json FROM postgis_eval.activities WHERE user_id=1 AND started_at >= $1::text::timestamptz AND id>$2 ORDER BY id LIMIT 16",&[&case.from,&after])?;
            if rows.is_empty() {
                break;
            }
            after = rows.last().unwrap().get("id");
            out.candidate_rows += rows.len();
            let started = Instant::now();
            for row in rows {
                let route = coords(&activity(&row)?);
                out.vertices += route.len();
                if route.len() >= 2 {
                    routes.push(route);
                }
            }
            out.decode_ms += started.elapsed().as_secs_f64() * 1000.0;
        }
        return Ok(routes);
    }
    let rows = if arm.ends_with('0') {
        if heatmap {
            out.access.query(db,"SELECT id,derived_data_json FROM postgis_eval.activities WHERE user_id=1 AND started_at >= $1::text::timestamptz ORDER BY id",&[&case.from])?
        } else {
            out.access.query(
                db,
                "SELECT * FROM postgis_eval.activities WHERE user_id=1 AND id=$1",
                &[&case.id],
            )?
        }
    } else {
        let select = if arm == "V1" {
            "coords"
        } else {
            "ST_AsBinary(geom,'NDR') AS coords"
        };
        if heatmap {
            let b = case.bbox.unwrap();
            if arm == "V1" {
                out.access.query(db,&format!("SELECT id,{select} FROM postgis_eval.activity_projection WHERE user_id=1 AND started_at>=$5::text::timestamptz AND maxx>=$1 AND minx<=$2 AND maxy>=$3 AND miny<=$4 ORDER BY id"),&[&b[0],&b[2],&b[1],&b[3],&case.from])?
            } else {
                out.access.query(db,&format!("SELECT id,{select} FROM postgis_eval.activity_projection WHERE user_id=1 AND started_at>=$5::text::timestamptz AND geom && ST_MakeEnvelope($1,$2,$3,$4,4326) ORDER BY id"),&[&b[0],&b[1],&b[2],&b[3],&case.from])?
            }
        } else {
            out.access.query(db,&format!("SELECT id,{select} FROM postgis_eval.activity_projection WHERE user_id=1 AND id=$1"),&[&case.id])?
        }
    };
    out.candidate_rows = rows.len();
    let started = Instant::now();
    let mut routes = Vec::new();
    for row in rows {
        let points = if arm.ends_with('0') {
            coords(&activity(&row)?)
        } else {
            geometry::decode_wkb(row.get::<_, Raw<'_>>("coords").0)?
        };
        out.vertices += points.len();
        if points.len() >= 2 {
            routes.push(points);
        }
    }
    out.decode_ms += started.elapsed().as_secs_f64() * 1000.0;
    Ok(routes)
}

pub fn execute(db: &mut Client, arm: &str, case: &Case, plans: bool) -> Result<Outcome> {
    let mut out = Outcome::default();
    out.access.collect_plans = plans;
    let value = match case.kind.as_str() {
        "detail" => {
            let rows = out.access.query(
                db,
                "SELECT * FROM postgis_eval.activities WHERE user_id=1 AND id=$1",
                &[&case.id],
            )?;
            ensure!(rows.len() == 1, "Missing owned activity");
            let efforts=out.access.query(db,"SELECT * FROM postgis_eval.segment_efforts WHERE user_id=1 AND activity_id=$1 ORDER BY id",&[&case.id])?;
            let started = Instant::now();
            let points = activity(&rows[0])?;
            out.vertices = points.len();
            out.decode_ms += started.elapsed().as_secs_f64() * 1000.0;
            out.candidate_rows = 1;
            let mut hash = Sha256::new();
            for row in rows.iter().chain(efforts.iter()) {
                for i in 0..row.len() {
                    let raw = row.get::<_, Option<Raw<'_>>>(i);
                    hash.update([u8::from(raw.is_some())]);
                    if let Some(raw) = raw {
                        hash.update((raw.0.len() as u64).to_le_bytes());
                        hash.update(raw.0);
                    }
                }
            }
            json!({"input_digest":format!("{:x}",hash.finalize()),"route":points})
        }
        "map_inputs" => {
            let routes = projected(db, arm, case, &mut out)?;
            json!({"points":routes.first().cloned().unwrap_or_default()})
        }
        "race" => race(db, case, &mut out)?,
        "segment_match" | "segment_history" => matching(db, arm, case, &mut out)?,
        "heatmap" => {
            let routes = projected(db, arm, case, &mut out)?;
            let started = Instant::now();
            // Candidate bbox includes gutter; raster uses the original tile bbox.
            let tile = case.tile_bbox.unwrap();
            let (png, covered) = geometry::heatmap(&routes, tile, case.size, case.gutter)?;
            out.domain_ms += started.elapsed().as_secs_f64() * 1000.0;
            out.output_bytes = png.len();
            out.fingerprint = geometry::hash(&png);
            let _ = covered;
            return Ok(out);
        }
        _ => anyhow::bail!("Unknown workload {}", case.kind),
    };
    let started = Instant::now();
    let bytes = serde_json::to_vec(&value)?;
    out.output_bytes = bytes.len();
    out.fingerprint = geometry::hash(&bytes);
    out.output_ms += started.elapsed().as_secs_f64() * 1000.0;
    Ok(out)
}
