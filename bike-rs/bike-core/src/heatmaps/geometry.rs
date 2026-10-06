use crate::activity_data::ActivityRoutePoint;

pub type Point = [f64; 2];
pub const WORLD_METERS: f64 = 40_075_016.686;
const MAX_PLAUSIBLE_SPEED_MPS: f64 = 45.0;

#[derive(Debug)]
pub struct Chunk {
    pub band: i32,
    pub bounds: [f64; 4],
    pub points: Vec<u8>,
}

pub fn project(longitude: f64, latitude: f64) -> Point {
    let latitude = latitude.clamp(-85.05112878, 85.05112878).to_radians();
    [
        (longitude + 180.0) / 360.0,
        (1.0 - latitude.tan().asinh() / std::f64::consts::PI) / 2.0,
    ]
}

pub fn unproject(point: Point) -> Point {
    [
        point[0] * 360.0 - 180.0,
        ((1.0 - point[1] * 2.0) * std::f64::consts::PI)
            .sinh()
            .atan()
            .to_degrees(),
    ]
}

pub fn encode(points: &[Point]) -> Vec<u8> {
    points
        .iter()
        .flat_map(|p| p.iter().flat_map(|n| n.to_le_bytes()))
        .collect()
}

pub fn decode(bytes: &[u8]) -> Option<Vec<Point>> {
    if bytes.len() < 32 || !bytes.len().is_multiple_of(16) {
        return None;
    }
    let points: Vec<Point> = bytes
        .chunks_exact(16)
        .map(|b| {
            [
                f64::from_le_bytes(b[..8].try_into().unwrap()),
                f64::from_le_bytes(b[8..].try_into().unwrap()),
            ]
        })
        .collect();
    points
        .iter()
        .flatten()
        .all(|n| n.is_finite() && (0.0..=1.0).contains(n))
        .then_some(points)
}

pub fn zoom_band(zoom: u8) -> i32 {
    match zoom {
        0..=7 => 0,
        8..=11 => 1,
        12..=15 => 2,
        _ => 3,
    }
}

pub fn prepare(route: &[ActivityRoutePoint]) -> Vec<Chunk> {
    let paths = continuous_paths(route);
    let mut chunks = Vec::new();
    for (band, max_zoom) in [7, 11, 15, 18].into_iter().enumerate() {
        let tolerance = 0.25 / (512.0 * 2f64.powi(max_zoom));
        for path in &paths {
            let simplified = simplify(path, tolerance);
            for start in (0..simplified.len().saturating_sub(1)).step_by(255) {
                let points = &simplified[start..(start + 256).min(simplified.len())];
                chunks.push(Chunk {
                    band: band as i32,
                    bounds: bounds(points),
                    points: encode(points),
                });
            }
        }
    }
    chunks
}

fn bounds(points: &[Point]) -> [f64; 4] {
    points.iter().fold([1.0, 1.0, 0.0, 0.0], |b, p| {
        [
            b[0].min(p[0]),
            b[1].min(p[1]),
            b[2].max(p[0]),
            b[3].max(p[1]),
        ]
    })
}

fn valid(p: &ActivityRoutePoint) -> bool {
    p.latitude.is_finite()
        && p.longitude.is_finite()
        && (-90.0..=90.0).contains(&p.latitude)
        && (-180.0..=180.0).contains(&p.longitude)
        && (p.latitude != 0.0 || p.longitude != 0.0)
}

fn continuous_paths(route: &[ActivityRoutePoint]) -> Vec<Vec<Point>> {
    let mut paths = Vec::new();
    let mut path = Vec::new();
    let mut previous: Option<&ActivityRoutePoint> = None;
    for p in route {
        if !valid(p) {
            finish(&mut path, &mut paths);
            previous = None;
            continue;
        }
        let next = project(p.longitude, p.latitude);
        if let Some(prev) = previous {
            let a = project(prev.longitude, prev.latitude);
            let mut dx = next[0] - a[0];
            dx -= dx.round();
            let meters =
                dx.hypot(next[1] - a[1]) * WORLD_METERS * p.latitude.to_radians().cos().abs();
            let dt = p.elapsed_seconds - prev.elapsed_seconds;
            if dt < 0 {
                finish(&mut path, &mut paths);
            } else if meters / f64::from(dt.max(1)) > MAX_PLAUSIBLE_SPEED_MPS {
                // Drop the suspect endpoint as well as the impossible segment. It may be a
                // single bad GPS fix; starting again at the next point avoids drawing from it.
                finish(&mut path, &mut paths);
                previous = None;
                continue;
            } else if meters > 5_000.0 || (dt >= 120 && meters > 200.0) {
                finish(&mut path, &mut paths);
            } else if (next[0] - a[0]).abs() > 0.5 {
                let wrapped = next[0] + if next[0] < a[0] { 1.0 } else { -1.0 };
                let edge = if wrapped > a[0] { 1.0 } else { 0.0 };
                let y = a[1] + (next[1] - a[1]) * (edge - a[0]) / (wrapped - a[0]);
                path.push([edge, y]);
                finish(&mut path, &mut paths);
                path.push([1.0 - edge, y]);
            }
        }
        if path.last() != Some(&next) {
            path.push(next);
        }
        previous = Some(p);
    }
    finish(&mut path, &mut paths);
    paths
}

fn finish(path: &mut Vec<Point>, paths: &mut Vec<Vec<Point>>) {
    if path.len() > 1 {
        paths.push(std::mem::take(path));
    } else {
        path.clear();
    }
}

fn simplify(points: &[Point], tolerance: f64) -> Vec<Point> {
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0, points.len() - 1)];
    while let Some((first, last)) = stack.pop() {
        let a = points[first];
        let b = points[last];
        let d = [b[0] - a[0], b[1] - a[1]];
        let length = d[0] * d[0] + d[1] * d[1];
        let mut furthest = first;
        let mut maximum = tolerance * tolerance;
        for (i, p) in points.iter().enumerate().take(last).skip(first + 1) {
            let t = if length == 0.0 {
                0.0
            } else {
                (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / length).clamp(0.0, 1.0)
            };
            let distance = (p[0] - a[0] - t * d[0]).powi(2) + (p[1] - a[1] - t * d[1]).powi(2);
            if distance > maximum {
                maximum = distance;
                furthest = i;
            }
        }
        if furthest != first {
            keep[furthest] = true;
            stack.extend([(first, furthest), (furthest, last)]);
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(p, keep)| keep.then_some(*p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(lon: f64, elapsed: i32) -> ActivityRoutePoint {
        ActivityRoutePoint {
            elapsed_seconds: elapsed,
            latitude: 45.0,
            longitude: lon,
            distance_meters: None,
            elevation_meters: None,
            speed_mps: None,
            heart_rate_bpm: None,
            cadence_rpm: None,
            power_watts: None,
        }
    }
    #[test]
    fn splits_gaps_and_dateline_without_world_spanning_lines() {
        let chunks = prepare(&[
            point(179.999, 0),
            point(-179.999, 10),
            point(-170.0, 11),
            point(-169.999, 21),
        ]);
        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.bounds[2] - c.bounds[0] < 0.001));
        assert!(chunks.iter().any(|c| c.bounds[2] == 1.0));
        assert!(chunks.iter().any(|c| c.bounds[0] == 0.0));
    }

    #[test]
    fn drops_impossible_speed_jumps_and_resumes_after_them() {
        let spike = point(-84.998, 11);
        let paths = continuous_paths(&[
            point(-85.001, 0),
            point(-85.0009, 10),
            spike.clone(),
            point(-85.0008, 12),
            point(-85.0007, 22),
        ]);

        assert_eq!(paths.len(), 2);
        assert!(paths
            .iter()
            .flatten()
            .all(|p| *p != project(spike.longitude, spike.latitude)));
        assert_eq!(paths[0].last(), Some(&project(-85.0009, 45.0)));
        assert_eq!(paths[1].first(), Some(&project(-85.0008, 45.0)));
    }

    #[test]
    fn binary_projection_roundtrips_and_simplifies_straight_routes() {
        let route: Vec<_> = (0..1000)
            .map(|i| point(-85.0 + f64::from(i) / 100_000.0, i))
            .collect();
        let chunks = prepare(&route);
        assert_eq!(chunks.len(), 4);
        assert!(chunks.iter().all(|c| decode(&c.points).unwrap().len() == 2));
        assert_eq!(
            decode(&encode(&[[0.123456789, 0.1234], [0.3, 0.4]])).unwrap(),
            vec![[0.123456789, 0.1234], [0.3, 0.4]]
        );
    }

    #[test]
    fn exact_two_minute_gps_gap_does_not_draw_a_two_kilometer_chord() {
        // Anonymized translation of the observed archive sample deltas. The
        // apparent speed is below 45 m/s, so only the inclusive gap rule catches it.
        let mut before = point(-120.0, 2);
        let mut after = point(-119.99143260531127, 122);
        after.latitude += 0.01734483987093;
        before.distance_meters = Some(500.0);
        after.distance_meters = Some(2312.69);
        let mut next = after.clone();
        next.elapsed_seconds += 2;
        next.longitude += 0.00001;
        let route = [point(-120.00001, 0), before, after, next];
        assert_eq!(continuous_paths(&route).len(), 2);
        let chunks = prepare(&route);
        assert_eq!(chunks.len(), 8);
        assert!(chunks.iter().all(|c| c.bounds[3] - c.bounds[1] < 0.000001));
    }

    #[test]
    fn real_continuously_recorded_long_road_is_not_removed() {
        let route: Vec<_> = (0..1500)
            .map(|i| point(-120.0 + f64::from(i) * 0.00003, i))
            .collect();
        assert_eq!(continuous_paths(&route).len(), 1);
        let chunks = prepare(&route);
        assert_eq!(chunks.len(), 4);
        assert!(chunks.iter().all(|c| decode(&c.points).unwrap().len() == 2));
        assert_eq!(
            decode(&chunks[3].points).unwrap().last(),
            Some(&project(route.last().unwrap().longitude, 45.0))
        );
    }
}
