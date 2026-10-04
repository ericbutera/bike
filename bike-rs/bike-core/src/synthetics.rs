//! An opt-in, isolated dataset for internal production browser checks.
use crate::auth::entities::users;
use crate::entities::{
    activities, segment_efforts, segment_summaries, segment_user_summaries, segments,
    synthetic_scenarios,
};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Set, TransactionTrait,
};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

pub const SCENARIO: &str = "platform-smoke/v1";
pub const AUTH_HEADER: &str = "x-bike-synthetic-key";

#[derive(Clone)]
pub struct SyntheticAuth {
    pub key: String,
    pub user_pid: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ScenarioManifest {
    pub name: String,
    pub activity_id: i32,
    pub segment_id: i32,
    pub race_effort_ids: [i32; 2],
}

impl From<synthetic_scenarios::Model> for ScenarioManifest {
    fn from(scenario: synthetic_scenarios::Model) -> Self {
        Self {
            name: scenario.name,
            activity_id: scenario.activity_id,
            segment_id: scenario.segment_id,
            race_effort_ids: [scenario.first_effort_id, scenario.second_effort_id],
        }
    }
}

pub async fn ensure_scenario(db: &DatabaseConnection) -> Result<users::Model, DbErr> {
    let tx = db.begin().await?;
    // Concurrent rolling starts provision once, without resetting any existing data.
    if tx.get_database_backend() == DbBackend::Postgres {
        tx.execute_unprepared("SELECT pg_advisory_xact_lock(743918260)")
            .await?;
    }
    let scenario = match synthetic_scenarios::Model::find(&tx, SCENARIO).await? {
        Some(scenario) => scenario,
        None => provision_scenario(&tx).await?,
    };
    let user = scenario.validate_owner(&tx).await?;
    tx.commit().await?;
    Ok(user)
}

pub async fn manifest(db: &DatabaseConnection, user_id: i32) -> Result<ScenarioManifest, DbErr> {
    let scenario = synthetic_scenarios::Model::find(db, SCENARIO)
        .await?
        .ok_or_else(|| DbErr::Custom("Synthetic scenario is not provisioned".into()))?;
    if scenario.user_id != user_id {
        return Err(DbErr::Custom("Synthetic scenario owner mismatch".into()));
    }
    scenario.validate_owner(db).await?;
    Ok(scenario.into())
}

async fn provision_scenario<C: ConnectionTrait>(
    db: &C,
) -> Result<synthetic_scenarios::Model, DbErr> {
    let now = Utc::now();
    // A conflicting reserved email fails insertion; never adopt an existing user.
    let user = users::ActiveModel {
        pid: Set(Uuid::new_v4()),
        email: Set("platform-smoke@synthetic.bike.invalid".into()),
        api_key: Set(Uuid::new_v4().to_string()),
        name: Set("Bike synthetic monitor".into()),
        oauth_provider: Set(Some(users::SYNTHETIC_PROVIDER.into())),
        is_admin: Set(Some(false)),
        disabled: Set(true),
        email_verified_at: Set(Some(now)),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let first = provision_ride(
        db,
        user.id,
        "Synthetic northbound A",
        12,
        "2025-06-01T12:00:00Z",
    )
    .await?;
    let second = provision_ride(
        db,
        user.id,
        "Synthetic northbound B",
        10,
        "2025-06-02T12:00:00Z",
    )
    .await?;
    let segment = segments::ActiveModel {
        user_id: Set(user.id),
        title: Set("Synthetic northbound segment".into()),
        source: Set("synthetic".into()),
        mode: Set("xc".into()),
        distance_meters: Set(Some(1000.0)),
        starred: Set(false),
        route_data_json: Set(Some(
            serde_json::from_value(route_points(12)).map_err(|e| DbErr::Custom(e.to_string()))?,
        )),
        last_activity_change_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let first_effort = provision_effort(db, &first, segment.id, 2).await?;
    let second_effort = provision_effort(db, &second, segment.id, 1).await?;
    provision_summaries(db, &segment, &second_effort, &second).await?;
    synthetic_scenarios::ActiveModel {
        name: Set(SCENARIO.into()),
        user_id: Set(user.id),
        activity_id: Set(first.id),
        segment_id: Set(segment.id),
        first_effort_id: Set(first_effort.id),
        second_effort_id: Set(second_effort.id),
    }
    .insert(db)
    .await
}

fn route_points(step: i32) -> serde_json::Value {
    json!((0..=10)
        .map(|i| json!({
            "elapsed_seconds": i * step, "latitude": 42.0 + f64::from(i) * 0.001,
            "longitude": -83.0, "distance_meters": i * 100, "elevation_meters": 200 + i * 2,
            "speed_mps": 100.0 / f64::from(step), "heart_rate_bpm": 140 + i
        }))
        .collect::<Vec<_>>())
}

async fn provision_ride<C: ConnectionTrait>(
    db: &C,
    user_id: i32,
    title: &str,
    step: i32,
    started_at: &str,
) -> Result<activities::Model, DbErr> {
    let started_at = started_at
        .parse::<DateTime<Utc>>()
        .map_err(|e| DbErr::Custom(e.to_string()))?;
    let points = route_points(step);
    activities::ActiveModel {
        user_id: Set(user_id),
        title: Set(title.into()),
        sport: Set("ride".into()),
        source: Set("synthetic".into()),
        activity_type: Set("training".into()),
        started_at: Set(started_at),
        ended_at: Set(Some(
            started_at + chrono::Duration::seconds(i64::from(step * 10)),
        )),
        distance_meters: Set(Some(1000.0)),
        moving_time_seconds: Set(Some(step * 10)),
        total_time_seconds: Set(Some(step * 10)),
        elevation_gain_meters: Set(Some(20.0)),
        average_speed_mps: Set(Some(100.0 / f64::from(step))),
        derived_data_json: Set(Some(
            serde_json::from_value(json!({"v": 2, "route_points": points, "chart_points": points}))
                .map_err(|e| DbErr::Custom(e.to_string()))?,
        )),
        ..Default::default()
    }
    .insert(db)
    .await
}

async fn provision_effort<C: ConnectionTrait>(
    db: &C,
    ride: &activities::Model,
    segment_id: i32,
    rank: i32,
) -> Result<segment_efforts::Model, DbErr> {
    let duration = ride.moving_time_seconds.unwrap_or_default();
    segment_efforts::ActiveModel {
        user_id: Set(ride.user_id),
        segment_id: Set(segment_id),
        activity_id: Set(ride.id),
        effort_index: Set(1),
        start_route_point_index: Set(0),
        end_route_point_index: Set(10),
        start_elapsed_seconds: Set(0),
        end_elapsed_seconds: Set(duration),
        duration_seconds: Set(duration),
        distance_meters: Set(Some(1000.0)),
        overall_rank: Set(Some(rank)),
        user_rank: Set(Some(rank)),
        ..Default::default()
    }
    .insert(db)
    .await
}

async fn provision_summaries<C: ConnectionTrait>(
    db: &C,
    segment: &segments::Model,
    best: &segment_efforts::Model,
    latest: &activities::Model,
) -> Result<(), DbErr> {
    segment_summaries::ActiveModel {
        segment_id: Set(segment.id),
        effort_count: Set(2),
        leader_user_id: Set(Some(segment.user_id)),
        leader_effort_id: Set(Some(best.id)),
        best_duration_seconds: Set(Some(best.duration_seconds)),
        latest_activity_started_at: Set(Some(latest.started_at)),
        latest_activity_id: Set(Some(latest.id)),
        latest_effort_id: Set(Some(best.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    segment_user_summaries::ActiveModel {
        segment_id: Set(segment.id),
        user_id: Set(segment.user_id),
        effort_count: Set(2),
        personal_best_effort_id: Set(Some(best.id)),
        personal_best_duration_seconds: Set(Some(best.duration_seconds)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}
