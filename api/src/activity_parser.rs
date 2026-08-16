use crate::activity_details::{derive_activity_detail_data, ActivityDerivedData};
use crate::activity_summary::{summarize_activity_upload, ActivityDraft};
use crate::app_error::AppError;
use crate::strava_provider_payload::parse_strava_provider_payload;

#[derive(Debug, Clone, PartialEq)]
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
    let draft = summarize_activity_upload(filename, format, bytes)?;
    let derived_data = derive_activity_detail_data(filename, format, bytes)?;

    Ok(ParsedActivityData {
        draft,
        derived_data,
    })
}
