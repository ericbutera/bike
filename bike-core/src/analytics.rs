use crate::activity_achievements::{
    ActivityAchievementHighlight, StoredActivityAchievementHighlights,
};
use crate::entities::{
    activities, activity_analytics, analytics_user_states, fitness_freshness_daily,
    segment_efforts, segment_summaries, segment_user_summaries, segments,
};
use crate::training_data::{
    deserialize_activity_heart_rate_zones, weighted_zone_intensity, StoredActivityHeartRateZones,
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set, TransactionSession, TransactionTrait,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivityAchievementKind {
    Kom,
    Top10,
    Pr,
    PersonalPodium,
}

trait ActivityTrainingLoadRowExt {
    fn training_load(&self) -> Option<f64>;
}

impl ActivityTrainingLoadRowExt for activities::ActivityTrainingLoadRow {
    fn training_load(&self) -> Option<f64> {
        estimated_training_load_from_fields(
            self.moving_time_seconds,
            self.total_time_seconds,
            self.average_heart_rate_bpm,
            self.max_heart_rate_bpm,
            self.heart_rate_zones_json.as_ref(),
        )
    }
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
    build_fitness_freshness_rows_from_loads(
        activities.iter().filter_map(|activity| {
            estimated_training_load(activity)
                .map(|training_load| (activity.started_at.date_naive(), training_load))
        }),
        start_date,
        end_date,
        initial_fitness,
        initial_fatigue,
    )
}

fn build_fitness_freshness_rows_from_loads(
    activity_loads: impl IntoIterator<Item = (NaiveDate, f64)>,
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_fitness: f64,
    initial_fatigue: f64,
) -> Vec<FitnessFreshnessDay> {
    let mut daily_load_by_date = BTreeMap::<NaiveDate, (i32, f64)>::new();

    for (day, training_load) in activity_loads {
        let entry = daily_load_by_date.entry(day).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += training_load;
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
    estimated_training_load_from_fields(
        activity.moving_time_seconds,
        activity.total_time_seconds,
        activity.average_heart_rate_bpm,
        activity.max_heart_rate_bpm,
        activity.heart_rate_zones_json.as_ref(),
    )
}

fn estimated_training_load_from_fields(
    moving_time_seconds: Option<i32>,
    total_time_seconds: Option<i32>,
    average_heart_rate_bpm: Option<i32>,
    max_heart_rate_bpm: Option<i32>,
    heart_rate_zones_json: Option<&StoredActivityHeartRateZones>,
) -> Option<f64> {
    let duration_seconds = moving_time_seconds
        .or(total_time_seconds)
        .filter(|value| *value > 0)?;
    let duration_hours = f64::from(duration_seconds) / 3600.0;
    let heart_rate_ratio = weighted_zone_intensity(&deserialize_activity_heart_rate_zones(
        heart_rate_zones_json,
    ))
    .unwrap_or_else(|| {
        estimated_heart_rate_ratio_from_fields(average_heart_rate_bpm, max_heart_rate_bpm)
    });

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
    let input = load_fitness_freshness_rebuild_input(db, user_id, end_date).await?;
    let rows = build_fitness_freshness_rows_for_rebuild(&input);
    let rebuilt_at = Utc::now();

    let txn = db.begin().await?;
    persist_fitness_freshness_rebuild(&txn, user_id, input, rows, rebuilt_at).await?;
    txn.commit().await
}

struct FitnessFreshnessRebuildInput {
    state: Option<analytics_user_states::Model>,
    dirty_from_day: Option<NaiveDate>,
    activity_rows: Vec<activities::ActivityTrainingLoadRow>,
    checkpoint_row: Option<fitness_freshness_daily::Model>,
    start_date: NaiveDate,
    end_date: NaiveDate,
}

async fn load_fitness_freshness_rebuild_input(
    db: &DatabaseConnection,
    user_id: i32,
    end_date: NaiveDate,
) -> Result<FitnessFreshnessRebuildInput, sea_orm::DbErr> {
    let state = analytics_user_states::Entity::find_by_id(user_id)
        .one(db)
        .await?;
    let dirty_from_day = state
        .as_ref()
        .and_then(|state| state.fitness_dirty_from_day);
    let activity_rows =
        activities::Model::list_training_loads_for_fitness_rebuild(db, user_id, dirty_from_day)
            .await?;
    let checkpoint_row = match dirty_from_day {
        Some(rebuild_from_day) => {
            fitness_freshness_daily::Model::latest_before_day(db, user_id, rebuild_from_day).await?
        }
        None => None,
    };
    let start_date = dirty_from_day.unwrap_or_else(|| {
        activity_rows
            .first()
            .map(|activity| activity.started_at.date_naive())
            .unwrap_or(end_date)
    });

    Ok(FitnessFreshnessRebuildInput {
        state,
        dirty_from_day,
        activity_rows,
        checkpoint_row,
        start_date,
        end_date,
    })
}

fn build_fitness_freshness_rows_for_rebuild(
    input: &FitnessFreshnessRebuildInput,
) -> Vec<FitnessFreshnessDay> {
    build_fitness_freshness_rows_from_loads(
        input.activity_rows.iter().filter_map(|activity| {
            activity
                .training_load()
                .map(|training_load| (activity.started_at.date_naive(), training_load))
        }),
        input.start_date,
        input.end_date,
        input
            .checkpoint_row
            .as_ref()
            .map(|row| row.fitness)
            .unwrap_or(0.0),
        input
            .checkpoint_row
            .as_ref()
            .map(|row| row.fatigue)
            .unwrap_or(0.0),
    )
}

async fn persist_fitness_freshness_rebuild<C>(
    db: &C,
    user_id: i32,
    input: FitnessFreshnessRebuildInput,
    rows: Vec<FitnessFreshnessDay>,
    rebuilt_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let row_models = fitness_freshness_active_models(user_id, rows, rebuilt_at);
    fitness_freshness_daily::Model::replace_user_rows_from_day(
        db,
        user_id,
        input.dirty_from_day,
        row_models,
    )
    .await?;
    mark_fitness_rebuild_complete(db, user_id, input.state, rebuilt_at).await
}

fn fitness_freshness_active_models(
    user_id: i32,
    rows: Vec<FitnessFreshnessDay>,
    rebuilt_at: DateTime<Utc>,
) -> Vec<fitness_freshness_daily::ActiveModel> {
    rows.into_iter()
        .map(|row| fitness_freshness_daily::ActiveModel {
            user_id: Set(user_id),
            day: Set(row.day),
            activity_count: Set(row.activity_count),
            training_load: Set(row.training_load),
            fitness: Set(row.fitness),
            fatigue: Set(row.fatigue),
            form: Set(row.form),
            created_at: Set(rebuilt_at),
            updated_at: Set(rebuilt_at),
            ..Default::default()
        })
        .collect()
}

async fn mark_fitness_rebuild_complete<C>(
    db: &C,
    user_id: i32,
    state: Option<analytics_user_states::Model>,
    rebuilt_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    if let Some(model) = state {
        let mut active_model: analytics_user_states::ActiveModel = model.into();
        active_model.fitness_dirty_from_day = Set(None);
        active_model.last_fitness_rebuild_at = Set(Some(rebuilt_at));
        active_model.update(db).await?;
        return Ok(());
    }

    analytics_user_states::ActiveModel {
        user_id: Set(user_id),
        last_activity_change_at: Set(rebuilt_at),
        fitness_dirty_from_day: Set(None),
        last_fitness_rebuild_at: Set(Some(rebuilt_at)),
        ..Default::default()
    }
    .insert(db)
    .await?;

    Ok(())
}

pub async fn rebuild_segment_analytics_cache<C>(
    db: &C,
    segment_ids: &[i32],
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let segment_ids = normalized_positive_ids(segment_ids);
    if segment_ids.is_empty() {
        return Ok(());
    }

    let input = load_segment_analytics_input(db, &segment_ids).await?;
    let activity_ids = input.activity_ids.clone();
    let rebuild = build_segment_analytics(input);

    let txn = db.begin().await?;
    persist_segment_analytics_rebuild(&txn, &segment_ids, rebuild).await?;
    rebuild_activity_analytics_cache(&txn, &activity_ids).await?;
    txn.commit().await
}

struct SegmentAnalyticsInput {
    efforts: Vec<segment_efforts::Model>,
    activity_ids: Vec<i32>,
    activity_started_at_by_id: HashMap<i32, DateTime<Utc>>,
}

struct SegmentEffortRankUpdate {
    effort: segment_efforts::Model,
    overall_rank: i32,
    user_rank: i32,
}

struct SegmentAnalyticsRebuild {
    segment_summary_by_id: HashMap<i32, SegmentSummaryAccumulator>,
    segment_user_summary_by_key: HashMap<(i32, i32), SegmentUserSummaryAccumulator>,
    effort_updates: Vec<SegmentEffortRankUpdate>,
}

async fn load_segment_analytics_input<C>(
    db: &C,
    segment_ids: &[i32],
) -> Result<SegmentAnalyticsInput, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let efforts = segment_efforts::Model::list_for_segment_analytics(db, segment_ids).await?;
    let activity_ids = unique_effort_activity_ids(&efforts);
    let activity_started_at_by_id = activities::Model::started_at_by_ids(db, &activity_ids).await?;

    Ok(SegmentAnalyticsInput {
        efforts,
        activity_ids,
        activity_started_at_by_id,
    })
}

fn build_segment_analytics(input: SegmentAnalyticsInput) -> SegmentAnalyticsRebuild {
    let mut rebuild = SegmentAnalyticsRebuild {
        segment_summary_by_id: HashMap::new(),
        segment_user_summary_by_key: HashMap::new(),
        effort_updates: Vec::with_capacity(input.efforts.len()),
    };
    let mut overall_ranks = HashMap::<i32, i32>::new();
    let mut user_ranks = HashMap::<(i32, i32), i32>::new();

    for effort in input.efforts {
        let overall_rank = next_rank(&mut overall_ranks, effort.segment_id);
        let user_rank = next_rank(&mut user_ranks, (effort.segment_id, effort.user_id));
        apply_segment_effort_to_rebuild(
            &mut rebuild,
            &input.activity_started_at_by_id,
            effort,
            overall_rank,
            user_rank,
        );
    }

    rebuild
}

fn next_rank<T>(ranks: &mut HashMap<T, i32>, key: T) -> i32
where
    T: Eq + std::hash::Hash,
{
    *ranks.entry(key).and_modify(|rank| *rank += 1).or_insert(1)
}

fn apply_segment_effort_to_rebuild(
    rebuild: &mut SegmentAnalyticsRebuild,
    activity_started_at_by_id: &HashMap<i32, DateTime<Utc>>,
    effort: segment_efforts::Model,
    overall_rank: i32,
    user_rank: i32,
) {
    update_segment_summary(
        rebuild
            .segment_summary_by_id
            .entry(effort.segment_id)
            .or_default(),
        &effort,
        activity_started_at_by_id.get(&effort.activity_id).copied(),
    );
    update_segment_user_summary(
        rebuild
            .segment_user_summary_by_key
            .entry((effort.segment_id, effort.user_id))
            .or_default(),
        &effort,
    );
    rebuild.effort_updates.push(SegmentEffortRankUpdate {
        effort,
        overall_rank,
        user_rank,
    });
}

fn update_segment_summary(
    summary: &mut SegmentSummaryAccumulator,
    effort: &segment_efforts::Model,
    activity_started_at: Option<DateTime<Utc>>,
) {
    summary.effort_count += 1;
    if summary.best_duration_seconds.is_none() {
        summary.best_duration_seconds = Some(effort.duration_seconds);
        summary.leader_user_id = Some(effort.user_id);
        summary.leader_effort_id = Some(effort.id);
    }

    let Some(started_at) = activity_started_at else {
        return;
    };
    if summary
        .latest_activity_started_at
        .is_some_and(|current| current >= started_at)
    {
        return;
    }

    summary.latest_activity_started_at = Some(started_at);
    summary.latest_activity_id = Some(effort.activity_id);
    summary.latest_effort_id = Some(effort.id);
}

fn update_segment_user_summary(
    summary: &mut SegmentUserSummaryAccumulator,
    effort: &segment_efforts::Model,
) {
    summary.effort_count += 1;
    if summary.personal_best_duration_seconds.is_none() {
        summary.personal_best_duration_seconds = Some(effort.duration_seconds);
        summary.personal_best_effort_id = Some(effort.id);
    }
}

async fn persist_segment_analytics_rebuild<C>(
    db: &C,
    segment_ids: &[i32],
    mut rebuild: SegmentAnalyticsRebuild,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    segment_user_summaries::Entity::delete_many()
        .filter(segment_user_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .exec(db)
        .await?;

    segment_summaries::Entity::delete_many()
        .filter(segment_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .exec(db)
        .await?;

    for update in rebuild.effort_updates {
        update_segment_effort_ranks(db, update).await?;
    }

    for segment_id in segment_ids.iter().copied() {
        let summary = rebuild
            .segment_summary_by_id
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
        .insert(db)
        .await?;
    }

    for ((segment_id, user_id), summary) in rebuild.segment_user_summary_by_key {
        segment_user_summaries::ActiveModel {
            segment_id: Set(segment_id),
            user_id: Set(user_id),
            effort_count: Set(summary.effort_count),
            personal_best_effort_id: Set(summary.personal_best_effort_id),
            personal_best_duration_seconds: Set(summary.personal_best_duration_seconds),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }

    Ok(())
}

async fn update_segment_effort_ranks<C>(
    db: &C,
    update: SegmentEffortRankUpdate,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let mut active_model: segment_efforts::ActiveModel = update.effort.into();
    active_model.overall_rank = Set(Some(update.overall_rank));
    active_model.user_rank = Set(Some(update.user_rank));
    active_model.update(db).await?;
    Ok(())
}

pub async fn rebuild_activity_analytics_cache<C>(
    db: &C,
    activity_ids: &[i32],
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let activity_ids = normalized_positive_ids(activity_ids);
    if activity_ids.is_empty() {
        return Ok(());
    }

    activity_analytics::Model::delete_by_activity_ids(db, &activity_ids).await?;
    let input = load_activity_analytics_input(db, &activity_ids).await?;
    if input.efforts.is_empty() {
        return Ok(());
    }

    let analytics_by_activity_id = build_activity_analytics(input);
    persist_activity_analytics(db, analytics_by_activity_id).await
}

struct ActivityAnalyticsInput {
    efforts: Vec<segment_efforts::Model>,
    segment_title_by_id: HashMap<i32, String>,
    personal_best_by_key: HashMap<(i32, i32), Option<i32>>,
}

async fn load_activity_analytics_input<C>(
    db: &C,
    activity_ids: &[i32],
) -> Result<ActivityAnalyticsInput, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let efforts = segment_efforts::Model::list_for_activity_analytics(db, activity_ids).await?;
    let segment_ids = unique_effort_segment_ids(&efforts);
    let user_ids = unique_effort_user_ids(&efforts);
    let segment_title_by_id = segment_titles_by_id(db, &segment_ids).await?;
    let personal_best_by_key = segment_personal_bests_by_key(db, &segment_ids, &user_ids).await?;

    Ok(ActivityAnalyticsInput {
        efforts,
        segment_title_by_id,
        personal_best_by_key,
    })
}

fn build_activity_analytics(
    input: ActivityAnalyticsInput,
) -> HashMap<i32, ActivityAnalyticsAccumulator> {
    let mut analytics_by_activity_id = HashMap::<i32, ActivityAnalyticsAccumulator>::new();

    for effort in input.efforts {
        apply_effort_to_activity_analytics(
            &mut analytics_by_activity_id,
            &input.segment_title_by_id,
            &input.personal_best_by_key,
            effort,
        );
    }

    analytics_by_activity_id
}

fn apply_effort_to_activity_analytics(
    analytics_by_activity_id: &mut HashMap<i32, ActivityAnalyticsAccumulator>,
    segment_title_by_id: &HashMap<i32, String>,
    personal_best_by_key: &HashMap<(i32, i32), Option<i32>>,
    effort: segment_efforts::Model,
) {
    let analytics = analytics_by_activity_id
        .entry(effort.activity_id)
        .or_default();
    analytics.user_id = Some(effort.user_id);
    analytics.segment_effort_count += 1;

    let Some(kind) = activity_achievement_kind(effort.overall_rank, effort.user_rank) else {
        return;
    };

    analytics.achievement_count += 1;
    apply_achievement_count(analytics, kind);
    analytics
        .achievement_highlights
        .push(activity_achievement_highlight(
            &effort,
            segment_title_by_id,
            personal_best_by_key,
        ));
}

fn apply_achievement_count(
    analytics: &mut ActivityAnalyticsAccumulator,
    kind: ActivityAchievementKind,
) {
    match kind {
        ActivityAchievementKind::Kom => analytics.kom_count += 1,
        ActivityAchievementKind::Top10 => analytics.top_10_count += 1,
        ActivityAchievementKind::Pr => analytics.pr_count += 1,
        ActivityAchievementKind::PersonalPodium => {}
    }
}

fn activity_achievement_highlight(
    effort: &segment_efforts::Model,
    segment_title_by_id: &HashMap<i32, String>,
    personal_best_by_key: &HashMap<(i32, i32), Option<i32>>,
) -> ActivityAchievementHighlight {
    ActivityAchievementHighlight {
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
    }
}

async fn persist_activity_analytics<C>(
    db: &C,
    analytics_by_activity_id: HashMap<i32, ActivityAnalyticsAccumulator>,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
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

async fn segment_titles_by_id<C>(
    db: &C,
    segment_ids: &[i32],
) -> Result<HashMap<i32, String>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    Ok(segments::Model::list_by_ids(db, segment_ids)
        .await?
        .into_iter()
        .map(|segment| (segment.id, segment.title))
        .collect())
}

async fn segment_personal_bests_by_key<C>(
    db: &C,
    segment_ids: &[i32],
    user_ids: &[i32],
) -> Result<HashMap<(i32, i32), Option<i32>>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    Ok(
        segment_user_summaries::Model::list_by_segment_and_user_ids(db, segment_ids, user_ids)
            .await?
            .into_iter()
            .map(|summary| {
                (
                    (summary.segment_id, summary.user_id),
                    summary.personal_best_duration_seconds,
                )
            })
            .collect(),
    )
}

fn normalized_positive_ids(ids: &[i32]) -> Vec<i32> {
    let mut ids = ids.iter().copied().filter(|id| *id > 0).collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn unique_effort_activity_ids(efforts: &[segment_efforts::Model]) -> Vec<i32> {
    let ids = efforts
        .iter()
        .map(|effort| effort.activity_id)
        .collect::<Vec<_>>();
    normalized_positive_ids(&ids)
}

fn unique_effort_segment_ids(efforts: &[segment_efforts::Model]) -> Vec<i32> {
    let ids = efforts
        .iter()
        .map(|effort| effort.segment_id)
        .collect::<Vec<_>>();
    normalized_positive_ids(&ids)
}

fn unique_effort_user_ids(efforts: &[segment_efforts::Model]) -> Vec<i32> {
    let ids = efforts
        .iter()
        .map(|effort| effort.user_id)
        .collect::<Vec<_>>();
    normalized_positive_ids(&ids)
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

fn estimated_heart_rate_ratio_from_fields(
    average_heart_rate_bpm: Option<i32>,
    max_heart_rate_bpm: Option<i32>,
) -> f64 {
    match (average_heart_rate_bpm, max_heart_rate_bpm) {
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
            activity_type: "training".to_string(),
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
