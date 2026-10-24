use super::{Input, Report, Result};
use bike_core::entities::{segment_overall_achievements, segment_personal_achievements};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

#[derive(Deserialize)]
struct Effort {
    id: i32,
    user_id: i32,
    segment_id: i32,
    duration_seconds: i32,
}

pub(super) async fn verify(db: &sea_orm::DatabaseConnection, input: &Input, report: &mut Report) -> Result<()> {
    let owner = input.reference["activity"]["user_id"].as_i64().ok_or("Missing owner")? as i32;
    let matched: Vec<Effort> = serde_json::from_value(input.reference["efforts"].clone())?;
    let matched_segments: HashSet<_> = matched.into_iter().map(|effort| effort.segment_id).collect();
    let context = input
        .reference
        .get("segment_efforts")
        .unwrap_or(&input.reference["efforts"]);
    let efforts: Vec<Effort> = serde_json::from_value(context.clone())?;
    let mut by_segment: BTreeMap<i32, Vec<Effort>> = BTreeMap::new();
    for effort in efforts
        .into_iter()
        .filter(|effort| matched_segments.contains(&effort.segment_id))
    {
        by_segment.entry(effort.segment_id).or_default().push(effort);
    }
    for (id, mut rows) in by_segment {
        rows.sort_by_key(|effort| (effort.duration_seconds, effort.id));
        let expected_personal: Vec<_> = rows
            .iter()
            .filter(|effort| effort.user_id == owner)
            .take(3)
            .enumerate()
            .map(|(position, effort)| (position as i32 + 1, effort.id, effort.duration_seconds))
            .collect();
        let actual_personal = segment_personal_achievements::Model::for_user_segment(db, owner, id).await?;
        let actual: Vec<_> = actual_personal
            .iter()
            .map(|row| (row.position, row.effort_id, row.duration_seconds))
            .collect();
        report.check(
            format!("segment {id}: personal podium matches frozen effort context"),
            actual == expected_personal,
        );
        let mut athletes = HashSet::new();
        let expected_overall: Vec<_> = rows
            .iter()
            .filter(|effort| athletes.insert(effort.user_id))
            .take(10)
            .enumerate()
            .map(|(position, effort)| (position as i32 + 1, effort.user_id, effort.id, effort.duration_seconds))
            .collect();
        let actual_overall = segment_overall_achievements::Model::for_segment(db, id).await?;
        let actual: Vec<_> = actual_overall
            .iter()
            .map(|row| (row.position, row.user_id, row.effort_id, row.duration_seconds))
            .collect();
        report.check(
            format!("segment {id}: overall standings match frozen effort context"),
            actual == expected_overall,
        );
    }
    Ok(())
}
