use serde_json::Value;

pub const SCENARIOS: [(&str, &str); 7] = [
    ("activity_list", "/api/activities?page=1&per_page=20"),
    ("activity_detail", "/api/activities/1100"),
    ("activity_missing", "/api/activities/1002"),
    ("activity_stale", "/api/activities/1003"),
    ("activity_empty", "/api/activities/1001"),
    ("segment_summary", "/api/segments/1"),
    ("segment_comparison", "/api/segments/1/comparison"),
];

pub fn valid(scenario: &str, body: &Value) -> bool {
    match scenario {
        "activity_list" => body["metadata"]["total"] == 100 && valid_list(body),
        "activity_detail" => body["id"] == 1100 && valid_detail(body),
        "activity_missing" => body["id"] == 1002 && valid_detail(body),
        "activity_stale" => body["id"] == 1003 && valid_detail(body),
        "activity_empty" => body["id"] == 1001 && body.get("segment_efforts").is_none(),
        "segment_summary" => body["id"] == 1 && valid_summary(body),
        "segment_comparison" => body["segment_id"] == 1 && valid_comparison(body),
        _ => false,
    }
}

fn length(body: &Value, key: &str, count: usize) -> bool {
    body[key].as_array().is_some_and(|items| items.len() == count)
}

fn valid_list(body: &Value) -> bool {
    if !length(body, "data", 20) {
        return false;
    }
    body["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["id"].as_i64().is_some_and(|id| (1001..=1100).contains(&id)))
}

fn valid_detail(body: &Value) -> bool {
    if !length(body, "route_points", 201) {
        return false;
    }
    length(body, "segment_efforts", 2)
}

fn valid_summary(body: &Value) -> bool {
    body["effort_count"] == 198 && body["best_duration_seconds"] == 150
}

fn valid_comparison(body: &Value) -> bool {
    if !length(body, "route_points", 51) {
        return false;
    }
    if !length(body, "efforts", 198) {
        return false;
    }
    body["efforts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|effort| length(effort, "route_points", 51))
}
