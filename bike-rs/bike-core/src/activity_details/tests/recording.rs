use super::*;
use crate::activity_recording::RecordingEnvironment;
use crate::heatmaps::geometry;

#[test]
fn fit_derivation_keeps_virtual_evidence_and_real_routes() {
    for (bytes, excluded) in [
        (
            include_bytes!("../../../tests/fixtures/recording/virtual.fit").as_slice(),
            true,
        ),
        (
            include_bytes!("../../../tests/fixtures/recording/virtual-generic.fit").as_slice(),
            true,
        ),
        (
            include_bytes!("../../../tests/fixtures/recording/indoor.fit").as_slice(),
            true,
        ),
        (
            include_bytes!("../../../tests/fixtures/recording/outdoor.fit").as_slice(),
            false,
        ),
    ] {
        let derived = derive_activity_detail_data("archive.fit", "fit", bytes).unwrap();
        assert_eq!(derived.recording.excluded(), excluded);
        assert_eq!(derived.route_points.len(), 16);
        assert!(!geometry::prepare(&derived.route_points).is_empty());
    }
}

#[test]
fn gpx_recorder_and_tcx_creator_are_independent_of_generic_ride_type() {
    let gpx = "<gpx creator='Zwift'><trk><type>Ride</type><trkseg><trkpt lat='45' lon='-120'><time>2020-01-01T00:00:00Z</time></trkpt></trkseg></trk></gpx>";
    let tcx = "<TrainingCenterDatabase><Activities><Activity Sport='Ride'><Id>2020-01-01T00:00:00Z</Id><Lap StartTime='2020-01-01T00:00:00Z'><TotalTimeSeconds>10</TotalTimeSeconds><DistanceMeters>20</DistanceMeters><Track><Trackpoint><Time>2020-01-01T00:00:00Z</Time><Position><LatitudeDegrees>45</LatitudeDegrees><LongitudeDegrees>-120</LongitudeDegrees></Position></Trackpoint></Track></Lap><Creator><Name>Zwift</Name></Creator></Activity></Activities></TrainingCenterDatabase>";
    for (format, xml) in [("gpx", gpx), ("tcx", tcx)] {
        let derived = derive_activity_detail_data("route", format, xml.as_bytes()).unwrap();
        assert_eq!(derived.recording.environment, RecordingEnvironment::Virtual);
        let real = xml.replace("Zwift", "Garmin");
        assert!(
            !derive_activity_detail_data("route", format, real.as_bytes())
                .unwrap()
                .recording
                .excluded()
        );
    }
}
