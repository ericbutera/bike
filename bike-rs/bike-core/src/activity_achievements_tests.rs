use super::*;

#[test]
fn unit_happy_highlights_preserve_personal_best_in_storage() {
    let highlight = ActivityAchievementHighlight {
        segment_id: 7,
        segment_title: "Riverfront climb".into(),
        effort_index: 1,
        overall_rank: Some(3),
        personal_rank: Some(1),
        personal_best_duration_seconds: Some(355),
    };
    let stored = StoredActivityAchievementHighlights::from_items(vec![highlight.clone()]);
    let value = serde_json::to_value(&stored).unwrap();

    assert_eq!(value["v"], 1);
    assert_eq!(value["items"][0]["personal_best_duration_seconds"], 355);
    let restored: StoredActivityAchievementHighlights = serde_json::from_value(value).unwrap();
    assert_eq!(restored.items, vec![highlight]);
}

#[test]
fn unit_happy_unversioned_highlights_load_with_current_format() {
    let stored: StoredActivityAchievementHighlights =
        serde_json::from_value(serde_json::json!({"items": []})).unwrap();
    assert_eq!(stored, StoredActivityAchievementHighlights::default());
}
