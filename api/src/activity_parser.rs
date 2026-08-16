use crate::activity_details::{derive_activity_detail_data, ActivityDerivedData};
use crate::activity_summary::{summarize_activity_upload, ActivityDraft};
use crate::app_error::AppError;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedActivityData {
    pub draft: ActivityDraft,
    pub derived_data: ActivityDerivedData,
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
