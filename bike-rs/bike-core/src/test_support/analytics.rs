use crate::entities::{analytics_user_states, fitness_freshness_daily, segment_efforts};
pub fn analytics_user_state() -> analytics_user_states::Model {
    let at = super::timestamp();
    analytics_user_states::Model {
        user_id: 7,
        last_activity_change_at: at,
        fitness_dirty_from_day: None,
        last_fitness_rebuild_at: None,
        created_at: at,
        updated_at: at,
    }
}
pub fn fitness_day() -> fitness_freshness_daily::Model {
    let at = super::timestamp();
    fitness_freshness_daily::Model {
        id: 1,
        user_id: 7,
        day: at.date_naive(),
        activity_count: 1,
        training_load: 10.0,
        fitness: 1.0,
        fatigue: 2.0,
        form: -1.0,
        created_at: at,
        updated_at: at,
    }
}
pub fn segment_effort() -> segment_efforts::Model {
    let at = super::timestamp();
    segment_efforts::Model {
        id: 1,
        user_id: 7,
        segment_id: 1,
        activity_id: 42,
        effort_index: 1,
        start_route_point_index: 0,
        end_route_point_index: 1,
        start_elapsed_seconds: 0,
        end_elapsed_seconds: 60,
        duration_seconds: 60,
        distance_meters: Some(1000.0),
        overall_rank: None,
        user_rank: None,
        created_at: at,
        updated_at: at,
    }
}

pub fn segment_id_rows(ids: &[i32]) -> Vec<std::collections::BTreeMap<String, sea_orm::Value>> {
    ids.iter()
        .map(|id| [("segment_id".into(), (*id).into())].into())
        .collect()
}
pub fn new_fitness_day() -> fitness_freshness_daily::ActiveModel {
    let mut active: fitness_freshness_daily::ActiveModel = fitness_day().into();
    active.id = sea_orm::ActiveValue::NotSet;
    active
}
pub fn new_segment_effort() -> segment_efforts::ActiveModel {
    let mut active: segment_efforts::ActiveModel = segment_effort().into();
    active.id = sea_orm::ActiveValue::NotSet;
    active
}

pub fn activity_analytics() -> crate::entities::activity_analytics::Model {
    let activity = super::activity();
    crate::entities::activity_analytics::Model {
        activity_id: activity.id,
        user_id: activity.user_id,
        segment_effort_count: 1,
        achievement_count: 0,
        kom_count: 0,
        top_10_count: 0,
        pr_count: 0,
        achievement_highlights_json: None,
        created_at: super::timestamp(),
        updated_at: super::timestamp(),
    }
}

pub fn activity_id_row() -> std::collections::BTreeMap<String, sea_orm::Value> {
    [("id".into(), super::activity().id.into())].into()
}
