use std::{env, fs, path::Path};

fn between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let source = &source[source.find(start).expect("Rust baseline start changed")..];
    &source[..source.find(end).expect("Rust baseline end changed")]
}

fn main() {
    let rustc = std::process::Command::new(env::var("RUSTC").unwrap())
        .arg("--version")
        .output()
        .unwrap();
    println!(
        "cargo:rustc-env=PROBE_RUSTC={}",
        String::from_utf8(rustc.stdout).unwrap().trim()
    );
    println!(
        "cargo:rustc-env=PROBE_TARGET={}",
        env::var("TARGET").unwrap()
    );
    let core = Path::new("../../../bike-rs/bike-core/src");
    for name in [
        "activity_data.rs",
        "segment_support.rs",
        "activity_sport.rs",
    ] {
        println!("cargo:rerun-if-changed={}", core.join(name).display());
    }
    // Compile the owning Rust algorithm, not a second matcher. Named boundaries
    // deliberately fail closed when the upstream module changes shape.
    let data = fs::read_to_string(core.join("activity_data.rs")).unwrap();
    let data = data
        .lines()
        .filter(|line| !line.trim().starts_with("#[schema"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("use sea_orm::FromJsonQueryResult;", "")
        .replace("use utoipa::ToSchema;", "")
        .replace(", ToSchema", "")
        .replace(", FromJsonQueryResult", "");
    let matcher = fs::read_to_string(core.join("segment_support.rs")).unwrap();
    let constants = between(
        &matcher,
        "const MIN_ENDPOINT_THRESHOLD_METERS",
        "pub fn serialize_segment_route_points",
    )
    .replace(
        "struct MatchedSegmentEffort",
        "pub struct MatchedSegmentEffort",
    )
    .replace(
        "#[derive(Debug, Clone, PartialEq)]",
        "#[derive(Debug, Clone, PartialEq, serde::Serialize)]",
    );
    let slicing = between(
        &matcher,
        "pub fn slice_effort_route_points",
        "pub async fn clear_segment_efforts_for_activity",
    );
    let filter = between(
        &matcher,
        "fn activity_may_contain_segment",
        "async fn insert_matches",
    );
    let algorithm = &matcher[matcher.find("fn match_segment_efforts(").unwrap()..];
    let sport = fs::read_to_string(core.join("activity_sport.rs")).unwrap();
    let sports = between(&sport, "pub const BIKE_ACTIVITY_SPORT_VALUES", "];");
    let output = format!(
        "use crate::activity_data::ActivityRoutePoint;\n{constants}\n{slicing}\n{filter}\n{algorithm}\n{sports}];\n\
         pub fn run_match(segment: &[ActivityRoutePoint], activity: &[ActivityRoutePoint]) -> Vec<MatchedSegmentEffort> {{\n\
         if activity_may_contain_segment(segment, activity) {{ match_segment_efforts(segment, activity) }} else {{ vec![] }}\n}}\n\
         pub fn minimum_prefilter_radius() -> f64 {{ MAX_ENDPOINT_THRESHOLD_METERS.max(FALLBACK_MAX_ENDPOINT_THRESHOLD_METERS).max(REWORKED_MAX_ENDPOINT_THRESHOLD_METERS) * 1.01 }}\n"
    );
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("baseline.rs"),
        output,
    )
    .unwrap();
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("activity_data.rs"),
        data,
    )
    .unwrap();
}
