use crate::activity_details::{derive_activity_detail_data, ActivityDerivedData};
use crate::activity_summary::{summarize_activity_upload, ActivityDraft};
use crate::strava_provider_payload::parse_strava_provider_payload;
use crate::workflow_error::WorkflowError as AppError;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParsedActivityData {
    pub draft: ActivityDraft,
    pub derived_data: ActivityDerivedData,
}

pub struct ActivityParserArtifact<'a> {
    pub original_filename: &'a str,
    pub format: &'a str,
    pub artifact_kind: &'a str,
    pub source_quality: &'a str,
    pub bytes: &'a [u8],
}

pub fn parse_activity_artifact(
    artifact: ActivityParserArtifact<'_>,
) -> Result<ParsedActivityData, AppError> {
    if artifact.artifact_kind == "generated_export" || artifact.source_quality == "generated_tcx" {
        return Err(retired_source_error());
    }
    if artifact.artifact_kind == "original" && artifact.source_quality == "strava_archive_json" {
        return crate::strava_archive_json::parse(artifact.bytes);
    }
    if artifact.artifact_kind == "provider_payload" && artifact.source_quality == "strava_streams" {
        return parse_strava_provider_payload(artifact.bytes);
    }

    parse_activity_data(artifact.original_filename, artifact.format, artifact.bytes)
}

pub fn parse_activity_data(
    filename: &str,
    format: &str,
    bytes: &[u8],
) -> Result<ParsedActivityData, AppError> {
    if format.eq_ignore_ascii_case("tcx") && is_retired_generated_tcx(bytes) {
        return Err(retired_source_error());
    }
    let draft = summarize_activity_upload(filename, format, bytes)?;
    let derived_data = derive_activity_detail_data(filename, format, bytes)?;

    Ok(ParsedActivityData {
        draft,
        derived_data,
    })
}

/// Recognize Bike's exact retired cycling export, including its nonstandard
/// `Ride` sport. Genuine TCX sources use their own encoding and remain supported.
pub fn is_retired_generated_tcx(bytes: &[u8]) -> bool {
    bytes.starts_with(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<TrainingCenterDatabase xmlns=\"http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2\" xmlns:ns3=\"http://www.garmin.com/xmlschemas/ActivityExtension/v2\">\n  <Activities>\n    <Activity Sport=\"Ride\">")
}

fn retired_source_error() -> AppError {
    AppError::bad_request(
        "Bike-generated activity exports are retired; recover an authentic source",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_labels_reject_even_parseable_bytes() {
        for (kind, quality) in [
            ("generated_export", "fit_original"),
            ("original", "generated_tcx"),
        ] {
            assert!(parse_activity_artifact(ActivityParserArtifact {
                original_filename: "synthetic.fit",
                format: "fit",
                artifact_kind: kind,
                source_quality: quality,
                bytes: include_bytes!("../tests/fixtures/recording/outdoor.fit"),
            })
            .unwrap_err()
            .to_string()
            .contains("retired"));
        }
    }

    #[test]
    fn manually_reuploaded_retired_bytes_are_rejected_despite_original_labels() {
        let bytes = include_bytes!("../tests/fixtures/recording/retired-copy.tcx");
        assert!(is_retired_generated_tcx(bytes));
        let error = parse_activity_artifact(ActivityParserArtifact {
            original_filename: "reuploaded.tcx",
            format: "tcx",
            artifact_kind: "original",
            source_quality: "tcx_original",
            bytes,
        })
        .unwrap_err();
        assert!(error.to_string().contains("retired"));
    }

    #[test]
    fn genuine_cycling_tcx_still_parses_and_contributes_geometry() {
        let bytes = include_bytes!("../tests/fixtures/recording/outdoor.tcx");
        assert!(!is_retired_generated_tcx(bytes));
        let parsed = parse_activity_artifact(ActivityParserArtifact {
            original_filename: "original.tcx",
            format: "tcx",
            artifact_kind: "original",
            source_quality: "tcx_original",
            bytes,
        })
        .unwrap();
        assert_eq!(parsed.derived_data.route_points.len(), 3);
        assert!(!crate::heatmaps::geometry::prepare(&parsed.derived_data.route_points).is_empty());
    }
}
