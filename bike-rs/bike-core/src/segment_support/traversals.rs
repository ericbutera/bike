use super::tests::route_point_at_offsets;
use super::*;

const POINTS_PER_LAP: i32 = 41;

fn segment_route(distance_meters: f64) -> Vec<ActivityRoutePoint> {
    (0..POINTS_PER_LAP)
        .map(|index| {
            let distance = f64::from(index) * distance_meters / 40.0;
            route_point_at_offsets(index * 15, distance, 0.0, distance)
        })
        .collect()
}

pub(super) fn lap_recording(laps: &[(f64, i32)]) -> Vec<ActivityRoutePoint> {
    let mut route = Vec::new();
    let mut starts_at = 0;
    for (lap, &(drift_meters, duration_seconds)) in laps.iter().enumerate() {
        for index in 0..POINTS_PER_LAP {
            let east = f64::from(index) * 50.0;
            route.push(route_point_at_offsets(
                starts_at + index * duration_seconds / 40,
                east,
                drift_meters,
                lap as f64 * 4000.0 + east,
            ));
        }
        starts_at += duration_seconds + 600;
    }
    route
}

fn recording_bounds(matches: &[MatchedSegmentEffort]) -> Vec<(i32, i32)> {
    matches
        .iter()
        .map(|effort| (effort.start_route_point_index, effort.end_route_point_index))
        .collect()
}

#[test]
fn six_mixed_profile_laps_keep_recording_order_including_the_slow_warmup() {
    let segment = segment_route(2000.0);
    let recording = lap_recording(&[
        (0.0, 600),
        (60.0, 540),
        (110.0, 480),
        (110.0, 420),
        (60.0, 360),
        (0.0, 300),
    ]);

    let matches = match_segment_efforts(&segment, &recording);

    assert_eq!(
        recording_bounds(&matches),
        vec![(0, 40), (41, 81), (82, 122), (123, 163), (164, 204), (205, 245)]
    );
    assert_eq!(
        matches.iter().map(|effort| effort.duration_seconds).collect::<Vec<_>>(),
        vec![600, 540, 480, 420, 360, 300]
    );
    assert_eq!(match_segment_efforts(&segment, &recording), matches);
}

#[test]
fn a_cleaner_later_fallback_lap_does_not_discard_the_earlier_valid_lap() {
    let matches = match_segment_efforts(&segment_route(2000.0), &lap_recording(&[(60.0, 600), (55.0, 480)]));

    assert_eq!(recording_bounds(&matches), vec![(0, 40), (41, 81)]);
}

#[test]
fn strictest_profile_keeps_its_original_bounds_without_duplicate_efforts() {
    let segment = segment_route(2000.0);
    let mut recording = lap_recording(&[(0.0, 600)]);
    for point in &mut recording {
        point.elapsed_seconds += 10;
        point.distance_meters = point.distance_meters.map(|distance| distance + 40.0);
    }
    recording.insert(0, route_point_at_offsets(0, -40.0, 0.0, 0.0));
    let strict = match_segment_efforts_with_profile(
        &segment,
        &recording,
        STRICT_MATCH_PROFILE,
        MatchSearchMode::FirstPassingStart,
    );
    let fallback = match_segment_efforts_with_profile(
        &segment,
        &recording,
        FALLBACK_MATCH_PROFILE,
        MatchSearchMode::BestPassingStart,
    );

    assert_eq!(recording_bounds(&strict), vec![(0, 41)]);
    assert_eq!(recording_bounds(&fallback), vec![(1, 41)]);
    assert_eq!(match_segment_efforts(&segment, &recording), strict);
}

#[test]
fn nested_segments_each_keep_all_six_overlapping_traversals() {
    let segment = segment_route(2000.0);
    let recording = lap_recording(&[(0.0, 600); 6]);

    let full = match_segment_efforts(&segment, &recording);
    let nested = match_segment_efforts(&segment[10..=30], &recording);

    assert_eq!(
        recording_bounds(&full),
        vec![(0, 40), (41, 81), (82, 122), (123, 163), (164, 204), (205, 245)]
    );
    assert_eq!(
        recording_bounds(&nested),
        vec![(9, 30), (50, 71), (91, 112), (132, 153), (173, 194), (214, 235)]
    );
}

#[test]
fn reverse_traversals_only_match_the_reversed_segment() {
    let segment = segment_route(2000.0);
    let mut reverse_segment = segment.clone();
    reverse_segment.reverse();
    let mut recording = lap_recording(&[(0.0, 600)]);
    let coordinates: Vec<_> = recording
        .iter()
        .rev()
        .map(|point| (point.latitude, point.longitude))
        .collect();
    for (point, (latitude, longitude)) in recording.iter_mut().zip(coordinates) {
        point.latitude = latitude;
        point.longitude = longitude;
    }

    assert!(match_segment_efforts(&segment, &recording).is_empty());
    assert_eq!(
        recording_bounds(&match_segment_efforts(&reverse_segment, &recording)),
        vec![(0, 40)]
    );
}

#[test]
fn short_segments_still_exclude_the_broad_reworked_trail_profile() {
    let segment = segment_route(1000.0);
    let recording: Vec<_> = (0..POINTS_PER_LAP)
        .map(|index| route_point_at_offsets(index * 15, f64::from(index) * 25.0, 110.0, f64::from(index) * 25.0))
        .collect();
    let broad = match_segment_efforts_with_profile(
        &segment,
        &recording,
        REWORKED_TRAIL_MATCH_PROFILE,
        MatchSearchMode::BestPassingStart,
    );

    assert_eq!(recording_bounds(&broad), vec![(0, 40)]);
    assert!(match_segment_efforts(&segment, &recording).is_empty());
}

fn fixture_route(directory: &str, filename: &str) -> Vec<ActivityRoutePoint> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/segment_matching")
        .join(directory)
        .join(filename);
    let bytes = std::fs::read(path).unwrap();
    let format = filename.rsplit('.').next().unwrap();
    crate::activity_parser::parse_activity_data(filename, format, &bytes)
        .unwrap()
        .derived_data
        .route_points
}

#[test]
fn favorite_segment_fixtures_match_expected_activity_files() {
    let cases: &[(&str, &[&str])] = &[
        (
            "unmarked_01.fit",
            &[
                "Segment - F-BOMB OUT.gpx",
                "Segment - Jam Sesh.gpx",
                "Segment - Adderall in Reverse.gpx",
                "Not-So Holy Grail (EB).gpx",
                "Segment - The Holy Grail Single Track.gpx",
            ],
        ),
        (
            "unmarked_02.fit",
            &[
                "VST Clockwise 2020 (6 to 5 - Saplings).gpx",
                "Segment - F-BOMB IN.gpx",
                "Segment - Rally Back CBS Rework.gpx",
            ],
        ),
        (
            "commons_01.fit",
            &[
                "Segment - Breakin The Law.gpx",
                "Segment - FMR.gpx",
                "Segment - FMR Full.gpx",
                "Log Jam.gpx",
            ],
        ),
        (
            "city_01.fit",
            &[
                "Segment - Breakin The Law.gpx",
                "Segment - -g.gpx",
                "Segment - The Hick's Descent.gpx",
                "Segment - Hickory Hills To Hickory Meadows.gpx",
            ],
        ),
        (
            "city_02.fit",
            &[
                "East Side Flow.gpx",
                "Country Club Boys.gpx",
                "Segment - -g.gpx",
                "Segment - The Hick's Descent.gpx",
                "Segment - Breakin The Law.gpx",
            ],
        ),
    ];
    for &(activity_file, segment_files) in cases {
        let recording = fixture_route("activities", activity_file);
        for segment_file in segment_files {
            let segment = fixture_route("segments", segment_file);
            assert!(
                !match_segment_efforts(&segment, &recording).is_empty(),
                "{activity_file} must match {segment_file}"
            );
        }
    }
}

#[test]
fn nearby_short_connector_fixture_does_not_become_an_effort() {
    let segment = fixture_route("segments", "short_connector_segment.gpx");
    let recording = fixture_route("activities", "short_connector_nearby_route.gpx");

    assert!(match_segment_efforts(&segment, &recording).is_empty());
}
