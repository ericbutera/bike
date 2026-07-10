use crate::activity_analytics::{
    ActivityAchievementHighlight, StoredActivityAchievementHighlights,
};
use crate::entities::{
    activities, activity_analytics, analytics_user_states, fitness_freshness_daily,
    segment_efforts, segment_summaries, segment_user_summaries, segments,
};
use crate::training_profile::{deserialize_activity_heart_rate_zones, weighted_zone_intensity};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Set, TransactionSession,
    TransactionTrait,
};
use std::collections::{BTreeMap, HashMap};

pub const FITNESS_WINDOW_DAYS: f64 = 42.0;
pub const FATIGUE_WINDOW_DAYS: f64 = 7.0;
const DEFAULT_HEART_RATE_RATIO: f64 = 0.6;

#[derive(Debug, Clone, PartialEq)]
pub struct FitnessFreshnessDay {
    pub day: NaiveDate,
    pub activity_count: i32,
    pub training_load: f64,
    pub fitness: f64,
    pub fatigue: f64,
    pub form: f64,
}

#[derive(Debug, Default)]
struct SegmentSummaryAccumulator {
    effort_count: i32,
    leader_user_id: Option<i32>,
    leader_effort_id: Option<i32>,
    best_duration_seconds: Option<i32>,
    latest_activity_started_at: Option<DateTime<Utc>>,
    latest_activity_id: Option<i32>,
    latest_effort_id: Option<i32>,
}

#[derive(Debug, Default)]
struct SegmentUserSummaryAccumulator {
    effort_count: i32,
    personal_best_effort_id: Option<i32>,
    personal_best_duration_seconds: Option<i32>,
}

#[derive(Debug, Default)]
struct ActivityAnalyticsAccumulator {
    user_id: Option<i32>,
    segment_effort_count: i32,
    achievement_count: i32,
    kom_count: i32,
    top_10_count: i32,
    pr_count: i32,
    achievement_highlights: Vec<ActivityAchievementHighlight>,
}

#[derive(Clone, Debug, FromQueryResult)]
struct ActivityStartedAtRow {
    id: i32,
    started_at: DateTime<Utc>,
}

#[derive(Clone, Debug, FromQueryResult)]
struct SegmentTitleRow {
    id: i32,
    title: String,
}

#[derive(Clone, Debug, FromQueryResult)]
struct SegmentPersonalBestRow {
    segment_id: i32,
    user_id: i32,
    personal_best_duration_seconds: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivityAchievementKind {
    Kom,
    Top10,
    Pr,
    PersonalPodium,
}

pub fn default_fitness_rebuild_start_date(
    activities: &[activities::Model],
    end_date: NaiveDate,
) -> NaiveDate {
    activities
        .first()
        .map(|activity| activity.started_at.date_naive())
        .unwrap_or(end_date)
}

pub fn build_fitness_freshness_rows(
    activities: &[activities::Model],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<FitnessFreshnessDay> {
    build_fitness_freshness_rows_with_seed(activities, start_date, end_date, 0.0, 0.0)
}

pub fn build_fitness_freshness_rows_with_seed(
    activities: &[activities::Model],
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_fitness: f64,
    initial_fatigue: f64,
) -> Vec<FitnessFreshnessDay> {
    let mut daily_load_by_date = BTreeMap::<NaiveDate, (i32, f64)>::new();

    for activity in activities {
        if let Some(training_load) = estimated_training_load(activity) {
            let entry = daily_load_by_date
                .entry(activity.started_at.date_naive())
                .or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += training_load;
        }
    }

    let mut current_date = start_date;
    let mut fitness = initial_fitness;
    let mut fatigue = initial_fatigue;
    let mut rows = Vec::new();

    while current_date <= end_date {
        let (activity_count, training_load) = daily_load_by_date
            .get(&current_date)
            .copied()
            .unwrap_or((0, 0.0));
        fitness += (training_load - fitness) / FITNESS_WINDOW_DAYS;
        fatigue += (training_load - fatigue) / FATIGUE_WINDOW_DAYS;
        let form = fitness - fatigue;

        rows.push(FitnessFreshnessDay {
            day: current_date,
            activity_count,
            training_load,
            fitness,
            fatigue,
            form,
        });

        current_date += Duration::days(1);
    }

    rows
}

pub fn estimated_training_load(activity: &activities::Model) -> Option<f64> {
    let duration_seconds = activity
        .moving_time_seconds
        .or(activity.total_time_seconds)
        .filter(|value| *value > 0)?;
    let duration_hours = f64::from(duration_seconds) / 3600.0;
    let heart_rate_ratio = weighted_zone_intensity(&deserialize_activity_heart_rate_zones(
        activity.heart_rate_zones_json.as_ref(),
    ))
    .unwrap_or_else(|| estimated_heart_rate_ratio(activity));

    Some(duration_hours * 100.0 * heart_rate_ratio.powi(2))
}

pub async fn mark_user_activity_change<C>(
    db: &C,
    user_id: i32,
    changed_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    if let Some(model) = analytics_user_states::Entity::find_by_id(user_id)
        .one(db)
        .await?
    {
        let mut active_model: analytics_user_states::ActiveModel = model.into();
        active_model.last_activity_change_at = Set(changed_at);
        active_model.update(db).await?;
    } else {
        analytics_user_states::ActiveModel {
            user_id: Set(user_id),
            last_activity_change_at: Set(changed_at),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }

    Ok(())
}

pub async fn mark_user_fitness_dirty<C>(
    db: &C,
    user_id: i32,
    dirty_from_day: NaiveDate,
    changed_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    if let Some(model) = analytics_user_states::Entity::find_by_id(user_id)
        .one(db)
        .await?
    {
        let next_dirty_from_day = model
            .fitness_dirty_from_day
            .map(|existing| existing.min(dirty_from_day))
            .unwrap_or(dirty_from_day);
        let mut active_model: analytics_user_states::ActiveModel = model.into();
        active_model.last_activity_change_at = Set(changed_at);
        active_model.fitness_dirty_from_day = Set(Some(next_dirty_from_day));
        active_model.update(db).await?;
    } else {
        analytics_user_states::ActiveModel {
            user_id: Set(user_id),
            last_activity_change_at: Set(changed_at),
            fitness_dirty_from_day: Set(Some(dirty_from_day)),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }

    Ok(())
}

pub async fn mark_user_activity_changes<C>(
    db: &C,
    user_ids: &[i32],
    changed_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let mut user_ids = user_ids
        .iter()
        .copied()
        .filter(|user_id| *user_id > 0)
        .collect::<Vec<_>>();
    user_ids.sort_unstable();
    user_ids.dedup();

    for user_id in user_ids {
        mark_user_activity_change(db, user_id, changed_at).await?;
    }

    Ok(())
}

pub async fn mark_segment_activity_changes<C>(
    db: &C,
    segment_ids: &[i32],
    changed_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let mut segment_ids = segment_ids
        .iter()
        .copied()
        .filter(|segment_id| *segment_id > 0)
        .collect::<Vec<_>>();
    segment_ids.sort_unstable();
    segment_ids.dedup();

    for segment_id in segment_ids {
        if let Some(model) = segments::Entity::find_by_id(segment_id).one(db).await? {
            let mut active_model: segments::ActiveModel = model.into();
            active_model.last_activity_change_at = Set(changed_at);
            active_model.update(db).await?;
        }
    }

    Ok(())
}

pub async fn rebuild_fitness_freshness_cache(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<(), sea_orm::DbErr> {
    let end_date = Utc::now().date_naive();
    let freshness_state = analytics_user_states::Entity::find_by_id(user_id)
        .one(db)
        .await?;
    let dirty_from_day = freshness_state
        .as_ref()
        .and_then(|state| state.fitness_dirty_from_day);

    let mut activity_query = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .order_by_asc(activities::Column::StartedAt);

    if let Some(rebuild_from_day) = dirty_from_day {
        let start_bound = DateTime::<Utc>::from_naive_utc_and_offset(
            rebuild_from_day
                .and_hms_opt(0, 0, 0)
                .expect("valid start of day"),
            Utc,
        );
        activity_query = activity_query.filter(activities::Column::StartedAt.gte(start_bound));
    }

    let activity_models = activity_query.all(db).await?;
    let checkpoint_row = if let Some(rebuild_from_day) = dirty_from_day {
        fitness_freshness_daily::Entity::find()
            .filter(fitness_freshness_daily::Column::UserId.eq(user_id))
            .filter(fitness_freshness_daily::Column::Day.lt(rebuild_from_day))
            .order_by_desc(fitness_freshness_daily::Column::Day)
            .one(db)
            .await?
    } else {
        None
    };
    let start_date = dirty_from_day
        .unwrap_or_else(|| default_fitness_rebuild_start_date(&activity_models, end_date));
    let rows = build_fitness_freshness_rows_with_seed(
        &activity_models,
        start_date,
        end_date,
        checkpoint_row
            .as_ref()
            .map(|row| row.fitness)
            .unwrap_or(0.0),
        checkpoint_row
            .as_ref()
            .map(|row| row.fatigue)
            .unwrap_or(0.0),
    );
    let rebuilt_at = Utc::now();

    let txn = db.begin().await?;

    let mut delete_query = fitness_freshness_daily::Entity::delete_many()
        .filter(fitness_freshness_daily::Column::UserId.eq(user_id));

    if let Some(rebuild_from_day) = dirty_from_day {
        delete_query =
            delete_query.filter(fitness_freshness_daily::Column::Day.gte(rebuild_from_day));
    }

    delete_query.exec(&txn).await?;

    for row in rows {
        fitness_freshness_daily::ActiveModel {
            user_id: Set(user_id),
            day: Set(row.day),
            activity_count: Set(row.activity_count),
            training_load: Set(row.training_load),
            fitness: Set(row.fitness),
            fatigue: Set(row.fatigue),
            form: Set(row.form),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
    }

    if let Some(model) = freshness_state {
        let mut active_model: analytics_user_states::ActiveModel = model.into();
        active_model.fitness_dirty_from_day = Set(None);
        active_model.last_fitness_rebuild_at = Set(Some(rebuilt_at));
        active_model.update(&txn).await?;
    } else {
        analytics_user_states::ActiveModel {
            user_id: Set(user_id),
            last_activity_change_at: Set(rebuilt_at),
            fitness_dirty_from_day: Set(None),
            last_fitness_rebuild_at: Set(Some(rebuilt_at)),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
    }

    txn.commit().await
}

pub async fn rebuild_segment_analytics_cache<C>(
    db: &C,
    segment_ids: &[i32],
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut segment_ids = segment_ids
        .iter()
        .copied()
        .filter(|segment_id| *segment_id > 0)
        .collect::<Vec<_>>();
    segment_ids.sort_unstable();
    segment_ids.dedup();

    if segment_ids.is_empty() {
        return Ok(());
    }

    let efforts = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .order_by_asc(segment_efforts::Column::SegmentId)
        .order_by_asc(segment_efforts::Column::DurationSeconds)
        .order_by_asc(segment_efforts::Column::Id)
        .all(db)
        .await?;
    let mut activity_ids = efforts
        .iter()
        .map(|effort| effort.activity_id)
        .collect::<Vec<_>>();
    activity_ids.sort_unstable();
    activity_ids.dedup();
    let activity_started_at_by_id = activities::Entity::find()
        .select_only()
        .column(activities::Column::Id)
        .column(activities::Column::StartedAt)
        .filter(activities::Column::Id.is_in(activity_ids.iter().copied()))
        .into_model::<ActivityStartedAtRow>()
        .all(db)
        .await?
        .into_iter()
        .map(|activity| (activity.id, activity.started_at))
        .collect::<HashMap<_, _>>();

    let mut overall_ranks = HashMap::<i32, i32>::new();
    let mut user_ranks = HashMap::<(i32, i32), i32>::new();
    let mut segment_summary_by_id = HashMap::<i32, SegmentSummaryAccumulator>::new();
    let mut segment_user_summary_by_key =
        HashMap::<(i32, i32), SegmentUserSummaryAccumulator>::new();
    let mut effort_updates = Vec::with_capacity(efforts.len());

    for effort in efforts {
        let overall_rank = overall_ranks
            .entry(effort.segment_id)
            .and_modify(|rank| *rank += 1)
            .or_insert(1);
        let user_rank = user_ranks
            .entry((effort.segment_id, effort.user_id))
            .and_modify(|rank| *rank += 1)
            .or_insert(1);

        let segment_summary = segment_summary_by_id.entry(effort.segment_id).or_default();
        segment_summary.effort_count += 1;
        if segment_summary.best_duration_seconds.is_none() {
            segment_summary.best_duration_seconds = Some(effort.duration_seconds);
            segment_summary.leader_user_id = Some(effort.user_id);
            segment_summary.leader_effort_id = Some(effort.id);
        }
        if let Some(started_at) = activity_started_at_by_id.get(&effort.activity_id).copied() {
            match segment_summary.latest_activity_started_at {
                Some(current) if current >= started_at => {}
                _ => {
                    segment_summary.latest_activity_started_at = Some(started_at);
                    segment_summary.latest_activity_id = Some(effort.activity_id);
                    segment_summary.latest_effort_id = Some(effort.id);
                }
            }
        }

        let segment_user_summary = segment_user_summary_by_key
            .entry((effort.segment_id, effort.user_id))
            .or_default();
        segment_user_summary.effort_count += 1;
        if segment_user_summary
            .personal_best_duration_seconds
            .is_none()
        {
            segment_user_summary.personal_best_duration_seconds = Some(effort.duration_seconds);
            segment_user_summary.personal_best_effort_id = Some(effort.id);
        }

        effort_updates.push((effort, *overall_rank, *user_rank));
    }

    let txn = db.begin().await?;

    segment_user_summaries::Entity::delete_many()
        .filter(segment_user_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .exec(&txn)
        .await?;

    segment_summaries::Entity::delete_many()
        .filter(segment_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .exec(&txn)
        .await?;

    for (effort, overall_rank, user_rank) in effort_updates {
        let mut active_model: segment_efforts::ActiveModel = effort.into();
        active_model.overall_rank = Set(Some(overall_rank));
        active_model.user_rank = Set(Some(user_rank));
        active_model.update(&txn).await?;
    }

    for segment_id in segment_ids.iter().copied() {
        let summary = segment_summary_by_id
            .remove(&segment_id)
            .unwrap_or_default();

        segment_summaries::ActiveModel {
            segment_id: Set(segment_id),
            effort_count: Set(summary.effort_count),
            leader_user_id: Set(summary.leader_user_id),
            leader_effort_id: Set(summary.leader_effort_id),
            best_duration_seconds: Set(summary.best_duration_seconds),
            latest_activity_started_at: Set(summary.latest_activity_started_at),
            latest_activity_id: Set(summary.latest_activity_id),
            latest_effort_id: Set(summary.latest_effort_id),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
    }

    for ((segment_id, user_id), summary) in segment_user_summary_by_key {
        segment_user_summaries::ActiveModel {
            segment_id: Set(segment_id),
            user_id: Set(user_id),
            effort_count: Set(summary.effort_count),
            personal_best_effort_id: Set(summary.personal_best_effort_id),
            personal_best_duration_seconds: Set(summary.personal_best_duration_seconds),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
    }

    rebuild_activity_analytics_cache(&txn, &activity_ids).await?;

    txn.commit().await
}

pub async fn rebuild_activity_analytics_cache<C>(
    db: &C,
    activity_ids: &[i32],
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let mut activity_ids = activity_ids
        .iter()
        .copied()
        .filter(|activity_id| *activity_id > 0)
        .collect::<Vec<_>>();
    activity_ids.sort_unstable();
    activity_ids.dedup();

    if activity_ids.is_empty() {
        return Ok(());
    }

    activity_analytics::Entity::delete_many()
        .filter(activity_analytics::Column::ActivityId.is_in(activity_ids.iter().copied()))
        .exec(db)
        .await?;

    let efforts = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::ActivityId.is_in(activity_ids.iter().copied()))
        .order_by_asc(segment_efforts::Column::ActivityId)
        .order_by_asc(segment_efforts::Column::DurationSeconds)
        .order_by_asc(segment_efforts::Column::Id)
        .all(db)
        .await?;

    if efforts.is_empty() {
        return Ok(());
    }

    let mut segment_ids = efforts
        .iter()
        .map(|effort| effort.segment_id)
        .collect::<Vec<_>>();
    segment_ids.sort_unstable();
    segment_ids.dedup();

    let mut user_ids = efforts
        .iter()
        .map(|effort| effort.user_id)
        .collect::<Vec<_>>();
    user_ids.sort_unstable();
    user_ids.dedup();

    let segment_title_by_id = segments::Entity::find()
        .select_only()
        .column(segments::Column::Id)
        .column(segments::Column::Title)
        .filter(segments::Column::Id.is_in(segment_ids.iter().copied()))
        .into_model::<SegmentTitleRow>()
        .all(db)
        .await?
        .into_iter()
        .map(|segment| (segment.id, segment.title))
        .collect::<HashMap<_, _>>();
    let personal_best_by_key = segment_user_summaries::Entity::find()
        .select_only()
        .column(segment_user_summaries::Column::SegmentId)
        .column(segment_user_summaries::Column::UserId)
        .column(segment_user_summaries::Column::PersonalBestDurationSeconds)
        .filter(segment_user_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .filter(segment_user_summaries::Column::UserId.is_in(user_ids.iter().copied()))
        .into_model::<SegmentPersonalBestRow>()
        .all(db)
        .await?
        .into_iter()
        .map(|summary| {
            (
                (summary.segment_id, summary.user_id),
                summary.personal_best_duration_seconds,
            )
        })
        .collect::<HashMap<_, _>>();

    let mut analytics_by_activity_id = HashMap::<i32, ActivityAnalyticsAccumulator>::new();

    for effort in efforts {
        let analytics = analytics_by_activity_id
            .entry(effort.activity_id)
            .or_default();
        analytics.user_id = Some(effort.user_id);
        analytics.segment_effort_count += 1;

        let Some(kind) = activity_achievement_kind(effort.overall_rank, effort.user_rank) else {
            continue;
        };

        analytics.achievement_count += 1;
        match kind {
            ActivityAchievementKind::Kom => analytics.kom_count += 1,
            ActivityAchievementKind::Top10 => analytics.top_10_count += 1,
            ActivityAchievementKind::Pr => analytics.pr_count += 1,
            ActivityAchievementKind::PersonalPodium => {}
        }

        analytics
            .achievement_highlights
            .push(ActivityAchievementHighlight {
                segment_id: effort.segment_id,
                segment_title: segment_title_by_id
                    .get(&effort.segment_id)
                    .cloned()
                    .unwrap_or_else(|| format!("Segment {}", effort.segment_id)),
                effort_index: effort.effort_index,
                overall_rank: effort.overall_rank,
                personal_rank: effort.user_rank,
                personal_best_duration_seconds: personal_best_by_key
                    .get(&(effort.segment_id, effort.user_id))
                    .copied()
                    .flatten(),
            });
    }

    for (activity_id, analytics) in analytics_by_activity_id {
        let Some(user_id) = analytics.user_id else {
            continue;
        };

        activity_analytics::ActiveModel {
            activity_id: Set(activity_id),
            user_id: Set(user_id),
            segment_effort_count: Set(analytics.segment_effort_count),
            achievement_count: Set(analytics.achievement_count),
            kom_count: Set(analytics.kom_count),
            top_10_count: Set(analytics.top_10_count),
            pr_count: Set(analytics.pr_count),
            achievement_highlights_json: Set(Some(
                StoredActivityAchievementHighlights::from_items(analytics.achievement_highlights),
            )),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }

    Ok(())
}

fn activity_achievement_kind(
    overall_rank: Option<i32>,
    personal_rank: Option<i32>,
) -> Option<ActivityAchievementKind> {
    if overall_rank == Some(1) {
        return Some(ActivityAchievementKind::Kom);
    }

    if overall_rank.is_some_and(|rank| (2..=10).contains(&rank)) {
        return Some(ActivityAchievementKind::Top10);
    }

    if personal_rank == Some(1) {
        return Some(ActivityAchievementKind::Pr);
    }

    if personal_rank.is_some_and(|rank| (2..=3).contains(&rank)) {
        return Some(ActivityAchievementKind::PersonalPodium);
    }

    None
}

fn estimated_heart_rate_ratio(activity: &activities::Model) -> f64 {
    match (activity.average_heart_rate_bpm, activity.max_heart_rate_bpm) {
        (Some(average), Some(maximum)) if maximum > 0 => {
            (f64::from(average) / f64::from(maximum)).clamp(0.35, 1.0)
        }
        (Some(average), _) => (f64::from(average) / 190.0).clamp(0.35, 1.0),
        _ => DEFAULT_HEART_RATE_RATIO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    fn make_activity(
        id: i32,
        started_at: &str,
        moving_time_seconds: Option<i32>,
        average_heart_rate_bpm: Option<i32>,
        max_heart_rate_bpm: Option<i32>,
    ) -> activities::Model {
        let timestamp = DateTime::parse_from_rfc3339(started_at)
            .unwrap()
            .with_timezone(&Utc);

        activities::Model {
            id,
            user_id: 1,
            activity_import_id: None,
            title: "Lunch Ride".to_string(),
            sport: "Ride".to_string(),
            source: "manual_upload".to_string(),
            source_correlation_id: None,
            original_filename: None,
            format: Some("fit".to_string()),
            activity_type: crate::activity_type::ActivityType::Training
                .as_str()
                .to_string(),
            started_at: timestamp,
            ended_at: None,
            distance_meters: Some(40000.0),
            moving_time_seconds,
            total_time_seconds: moving_time_seconds,
            elevation_gain_meters: Some(500.0),
            elevation_loss_meters: Some(500.0),
            average_speed_mps: Some(8.0),
            max_speed_mps: Some(12.0),
            average_heart_rate_bpm,
            max_heart_rate_bpm,
            average_cadence_rpm: Some(85),
            max_cadence_rpm: Some(105),
            calories: Some(850),
            estimated_ftp_watts: None,
            heart_rate_zones_json: None,
            derived_data_json: None,
            created_at: timestamp,
            updated_at: timestamp,
        }
    }

    #[test]
    fn builds_fitness_rows_with_decay_through_empty_days() {
        let activities = vec![
            make_activity(1, "2026-05-01T12:00:00Z", Some(3600), Some(120), Some(170)),
            make_activity(2, "2026-05-03T12:00:00Z", Some(5400), Some(155), Some(170)),
        ];

        let rows = build_fitness_freshness_rows(
            &activities,
            NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 5, 4).unwrap(),
        );

        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].activity_count, 1);
        assert_eq!(rows[1].activity_count, 0);
        assert_eq!(rows[2].activity_count, 1);
        assert_eq!(rows[3].activity_count, 0);
        assert!(rows[2].training_load > rows[0].training_load);
        assert!(rows[3].fatigue < rows[2].fatigue);
    }

    #[test]
    fn defaults_fitness_start_to_today_when_history_is_empty() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 7).unwrap();

        assert_eq!(default_fitness_rebuild_start_date(&[], today), today);
    }

    #[test]
    fn builds_fitness_rows_from_seed_state() {
        let activities = vec![make_activity(
            1,
            "2026-05-03T12:00:00Z",
            Some(3600),
            Some(150),
            Some(180),
        )];

        let rows = build_fitness_freshness_rows_with_seed(
            &activities,
            NaiveDate::from_ymd_opt(2026, 5, 3).unwrap(),
            NaiveDate::from_ymd_opt(2026, 5, 4).unwrap(),
            12.0,
            20.0,
        );

        assert_eq!(rows[0].day, NaiveDate::from_ymd_opt(2026, 5, 3).unwrap());
        assert!(rows[0].fitness > 12.0);
        assert!(rows[1].fatigue < rows[0].fatigue);
    }

    #[test]
    fn prefers_primary_activity_achievement_kinds() {
        assert_eq!(
            activity_achievement_kind(Some(1), Some(1)),
            Some(ActivityAchievementKind::Kom)
        );
        assert_eq!(
            activity_achievement_kind(Some(7), Some(1)),
            Some(ActivityAchievementKind::Top10)
        );
        assert_eq!(
            activity_achievement_kind(None, Some(1)),
            Some(ActivityAchievementKind::Pr)
        );
        assert_eq!(
            activity_achievement_kind(None, Some(2)),
            Some(ActivityAchievementKind::PersonalPodium)
        );
        assert_eq!(activity_achievement_kind(None, None), None);
    }
}
