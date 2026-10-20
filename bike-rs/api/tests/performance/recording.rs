use bike_core::activity_details::{ActivityChartPoint, ActivityDerivedData, ActivityRoutePoint};
use bike_core::test_support as fixtures;

pub fn recording(step: i32) -> ActivityDerivedData {
    let template = fixtures::parsed_reprocess_activity().derived_data.route_points[0].clone();
    let route_points: Vec<_> = (0..=200).map(|index| point(&template, index, step)).collect();
    let chart_points = route_points.iter().step_by(2).map(chart).collect();
    ActivityDerivedData {
        route_points,
        chart_points,
        ..Default::default()
    }
}

fn point(template: &ActivityRoutePoint, index: i32, step: i32) -> ActivityRoutePoint {
    let position = index % 100;
    let position = if position <= 50 { position } else { 100 - position };
    ActivityRoutePoint {
        elapsed_seconds: index * step,
        latitude: template.latitude + f64::from(position) * 0.0001,
        longitude: template.longitude,
        distance_meters: Some(f64::from(index) * 11.1195),
        elevation_meters: Some(200.0 + f64::from(position)),
        speed_mps: Some(11.1195 / f64::from(step)),
        heart_rate_bpm: Some(140 + index % 20),
        cadence_rpm: Some(80 + index % 10),
        power_watts: Some(180 + index % 40),
    }
}

fn chart(point: &ActivityRoutePoint) -> ActivityChartPoint {
    ActivityChartPoint {
        elapsed_seconds: point.elapsed_seconds,
        distance_meters: point.distance_meters,
        elevation_meters: point.elevation_meters,
        speed_mps: point.speed_mps,
        heart_rate_bpm: point.heart_rate_bpm,
        cadence_rpm: point.cadence_rpm,
        power_watts: point.power_watts,
    }
}
