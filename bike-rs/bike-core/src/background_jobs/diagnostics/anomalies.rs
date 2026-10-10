//! One versioned policy feeds the ledger, admin and Grafana events.
use super::super::{
    entities::{background_tasks, task_anomalies, task_attempts, work_units},
    worker::{catalog::ProcessorDefinition, WorkerMetrics},
};
use chrono::Utc;
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryFilter, Set,
};

const POLICY: i32 = 1;
const REASONS: &[&str] = &[
    "queue_budget",
    "runtime_budget",
    "heartbeat_stale",
    "progress_stale",
    "retry_exhausted",
    "clock_skew",
    "duration_outlier",
    "repeated_revision",
    "stale_publication",
];

pub(super) async fn evaluate(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
) -> Result<(), DbErr> {
    task_anomalies::Model::resolve_inapplicable(db).await?;
    // Every processor/state gets a bounded share; old failed tasks cannot
    // starve the running or queued processors behind them.
    let tasks = background_tasks::Model::diagnostic_candidates(db).await?;
    for task in tasks {
        evaluate_task(db, metrics, &task).await?;
    }
    for processor in super::super::entities::processor_registry::Entity::find()
        .all(db)
        .await?
    {
        for reason in REASONS {
            metrics.set_anomalies(&processor.task_type, reason, 0);
        }
    }
    for row in task_anomalies::Model::active_counts(db).await? {
        metrics.set_anomalies(&row.task_type, &row.reason, row.count);
    }
    Ok(())
}

async fn evaluate_task(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
    task: &background_tasks::Model,
) -> Result<(), DbErr> {
    let now = Utc::now();
    let policy = ProcessorDefinition::for_type(&task.task_type);
    let attempts = task_attempts::Model::for_task(db, task.id).await?;
    let attempt = attempts.last();
    let elapsed =
        |timestamp: chrono::DateTime<Utc>| (now - timestamp).num_milliseconds() as f64 / 1000.0;
    let signals = [
        (
            "queue_budget",
            elapsed(task.eligible_at()),
            policy.queue_budget_seconds as f64,
            task.status == "pending" && task.scheduled_for.is_none_or(|time| time <= now),
        ),
        (
            "runtime_budget",
            attempt.map(|a| elapsed(a.started_at)).unwrap_or(0.0),
            policy.runtime_budget_seconds as f64,
            task.status == "processing",
        ),
        (
            "heartbeat_stale",
            attempt.map(|a| elapsed(a.heartbeat_at)).unwrap_or(0.0),
            120.0,
            task.status == "processing",
        ),
        (
            "progress_stale",
            attempt.map(|a| elapsed(a.progress_at)).unwrap_or(0.0),
            policy.progress_budget_seconds as f64,
            task.status == "processing",
        ),
        (
            "retry_exhausted",
            f64::from(task.attempts),
            f64::from(task.max_attempts) - 1.0,
            task.status == "failed",
        ),
    ];
    for (reason, observed, expected, applicable) in signals {
        if applicable && observed > expected {
            record(
                db,
                metrics,
                task,
                reason,
                Evidence {
                    observed,
                    expected,
                    baseline_samples: 0,
                },
            )
            .await?;
        } else {
            resolve(db, task.id, task.attempts, reason).await?;
        }
    }
    Ok(())
}

pub(super) struct Evidence {
    pub observed: f64,
    pub expected: f64,
    pub baseline_samples: i64,
}
pub(super) async fn record(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
    task: &background_tasks::Model,
    reason: &str,
    evidence: Evidence,
) -> Result<(), DbErr> {
    let inserted = task_anomalies::Entity::insert(task_anomalies::ActiveModel {
        task_id: Set(task.id),
        attempt: Set(task.attempts),
        reason: Set(reason.into()),
        policy_version: Set(POLICY),
        observed: Set(evidence.observed),
        expected: Set(evidence.expected),
        baseline_samples: Set(evidence.baseline_samples),
        evaluated_at: Set(Utc::now()),
        resolved_at: Set(None),
        severity: Set("warning".into()),
    })
    .on_conflict(
        OnConflict::columns([
            task_anomalies::Column::TaskId,
            task_anomalies::Column::Attempt,
            task_anomalies::Column::Reason,
            task_anomalies::Column::PolicyVersion,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    let reactivated = if inserted == 0 {
        task_anomalies::Entity::update_many()
            .col_expr(
                task_anomalies::Column::ResolvedAt,
                Expr::value(None::<chrono::DateTime<Utc>>),
            )
            .col_expr(task_anomalies::Column::EvaluatedAt, Expr::value(Utc::now()))
            .col_expr(
                task_anomalies::Column::Observed,
                Expr::value(evidence.observed),
            )
            .filter(task_anomalies::Column::TaskId.eq(task.id))
            .filter(task_anomalies::Column::Attempt.eq(task.attempts))
            .filter(task_anomalies::Column::Reason.eq(reason))
            .filter(task_anomalies::Column::PolicyVersion.eq(POLICY))
            .filter(task_anomalies::Column::ResolvedAt.is_not_null())
            .exec(db)
            .await?
            .rows_affected
    } else {
        0
    };
    if inserted > 0 || reactivated > 0 {
        log_transition(db, metrics, task, reason, &evidence).await?;
    }
    Ok(())
}

async fn resolve(
    db: &DatabaseConnection,
    id: i32,
    attempt: i32,
    reason: &str,
) -> Result<(), DbErr> {
    task_anomalies::Entity::update_many()
        .col_expr(task_anomalies::Column::ResolvedAt, Expr::value(Utc::now()))
        .filter(task_anomalies::Column::TaskId.eq(id))
        .filter(task_anomalies::Column::Attempt.lte(attempt))
        .filter(task_anomalies::Column::Reason.eq(reason))
        .filter(task_anomalies::Column::ResolvedAt.is_null())
        .exec(db)
        .await?;
    Ok(())
}

pub async fn completed(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
    task: &background_tasks::Model,
) -> Result<(), DbErr> {
    evaluate_task(db, metrics, task).await?;
    evaluate_duration(db, metrics, task).await?;
    let units = work_units::Entity::find()
        .filter(work_units::Column::TaskId.eq(task.id))
        .filter(work_units::Column::Attempt.eq(task.attempts))
        .all(db)
        .await?;
    for unit in units {
        if unit.outcome == "superseded" {
            record(
                db,
                metrics,
                task,
                "stale_publication",
                Evidence {
                    observed: 1.0,
                    expected: 0.0,
                    baseline_samples: 0,
                },
            )
            .await?;
        }
        if unit.outcome != "published" {
            continue;
        }
        let previous = work_units::Entity::find()
            .filter(work_units::Column::Kind.eq(&unit.kind))
            .filter(work_units::Column::WorkKey.eq(&unit.work_key))
            .filter(work_units::Column::Revision.eq(&unit.revision))
            .filter(work_units::Column::ProcessingVersion.eq(&unit.processing_version))
            .filter(work_units::Column::Mode.eq(&unit.mode))
            .filter(work_units::Column::Outcome.eq("published"))
            .filter(work_units::Column::Id.ne(unit.id))
            .filter(work_units::Column::StartedAt.gte(unit.started_at - chrono::Duration::days(7)))
            .filter(work_units::Column::StartedAt.lte(unit.started_at))
            .count(db)
            .await?;
        if previous > 0 {
            record(
                db,
                metrics,
                task,
                "repeated_revision",
                Evidence {
                    observed: previous as f64 + 1.0,
                    expected: 1.0,
                    baseline_samples: 0,
                },
            )
            .await?;
        }
    }
    Ok(())
}

#[derive(sea_orm::FromQueryResult)]
struct Baseline {
    samples: i64,
    p90: Option<f64>,
}

async fn evaluate_duration(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
    task: &background_tasks::Model,
) -> Result<(), DbErr> {
    use sea_orm::{DbBackend, FromQueryResult, Statement};
    let attempts = task_attempts::Model::for_task(db, task.id).await?;
    let Some(attempt) = attempts.last() else {
        return Ok(());
    };
    if let Ok(Some(origin)) = super::super::pipeline::PipelineContext::from_payload(&task.payload) {
        let skew =
            (origin.pipeline_started_at - attempt.started_at).num_milliseconds() as f64 / 1000.0;
        if skew > 0.0 {
            record(
                db,
                metrics,
                task,
                "clock_skew",
                Evidence {
                    observed: skew,
                    expected: 0.0,
                    baseline_samples: 0,
                },
            )
            .await?;
        }
    }
    if attempt.outcome != "completed" || db.get_database_backend() != DbBackend::Postgres {
        return Ok(());
    }
    let units = work_units::Entity::find()
        .filter(work_units::Column::TaskId.eq(task.id))
        .filter(work_units::Column::Attempt.eq(task.attempts))
        .all(db)
        .await?;
    let Some(cohort) = work_cohort(&units) else {
        return Ok(());
    };
    task_attempts::Entity::update_many()
        .col_expr(task_attempts::Column::WorkloadCohort, Expr::value(&cohort))
        .filter(task_attempts::Column::TaskId.eq(task.id))
        .filter(task_attempts::Column::Attempt.eq(task.attempts))
        .exec(db)
        .await?;
    // Native ordered-set percentile is not represented by SeaORM aggregates.
    // Earlier comparable executions only; the candidate and future runs cannot bias its threshold.
    let baseline = Baseline::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT COUNT(*)::bigint samples, percentile_cont(0.9) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM (a.finished_at-a.started_at))) p90 FROM background_task_attempts a JOIN background_tasks t ON t.id=a.task_id WHERE t.task_type=$1 AND a.processing_version=$2 AND a.workload_cohort=$3 AND a.outcome='completed' AND a.started_at<$4 AND a.finished_at<=$4 AND a.started_at>=$5",
        vec![task.task_type.clone().into(), attempt.processing_version.clone().into(), cohort.into(), attempt.started_at.into(), (attempt.started_at-chrono::Duration::days(7)).into()]))
        .one(db).await?;
    let Some(Baseline {
        samples,
        p90: Some(p90),
    }) = baseline
    else {
        return Ok(());
    };
    if samples < 30 {
        return Ok(());
    }
    let expected =
        (p90 * 3.0).max(ProcessorDefinition::for_type(&task.task_type).outlier_floor_seconds);
    let seconds = attempt
        .finished_at
        .map(|end| (end - attempt.started_at).num_milliseconds() as f64 / 1000.0)
        .unwrap_or(0.0);
    if seconds > expected {
        record(
            db,
            metrics,
            task,
            "duration_outlier",
            Evidence {
                observed: seconds,
                expected,
                baseline_samples: samples,
            },
        )
        .await?;
    }
    Ok(())
}

fn work_cohort(units: &[work_units::Model]) -> Option<String> {
    let first = units.first()?;
    if units.iter().any(|unit| {
        unit.kind != first.kind || unit.mode != first.mode || unit.outcome != "published"
    }) {
        return None;
    }
    let counts = units
        .iter()
        .try_fold((0u64, 0u64), |(read, compute), unit| {
            let counts = unit.counts.as_ref()?;
            Some((
                read + counts["inputs_read"].as_u64()?,
                compute + counts["units_computed"].as_u64()?,
            ))
        })?;
    Some(format!(
        "{}:{}:read{}:compute{}",
        first.kind,
        first.mode,
        count_band(counts.0),
        count_band(counts.1)
    ))
}
fn count_band(count: u64) -> u32 {
    if count == 0 {
        0
    } else {
        count.ilog10() + 1
    }
}

async fn log_transition(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
    task: &background_tasks::Model,
    reason: &str,
    evidence: &Evidence,
) -> Result<(), DbErr> {
    // Correlation comes from compact durable lineage, including shared tasks;
    // the periodic sampler does not need to read a worker's source payload.
    use super::super::entities::pipeline_tasks;
    use sea_orm::QueryOrder;
    let origin = pipeline_tasks::Entity::find()
        .filter(pipeline_tasks::Column::TaskId.eq(task.id))
        .order_by_asc(pipeline_tasks::Column::RunId)
        .find_also_related(super::super::entities::pipeline_runs::Entity)
        .one(db)
        .await?
        .and_then(|(_, origin)| origin);
    let attempts = task_attempts::Model::for_task(db, task.id).await?;
    let trace_id = attempts
        .last()
        .and_then(|attempt| attempt.trace_id.as_deref())
        .unwrap_or("");
    metrics.record_anomaly(&task.task_type, reason);
    tracing::warn!(
        event = "worker_anomaly",
        task_id = task.id,
        run_id = origin
            .as_ref()
            .map(|origin| origin.id.as_str())
            .unwrap_or(""),
        request_id = origin
            .as_ref()
            .and_then(|origin| origin.request_id.as_deref())
            .unwrap_or(""),
        trace_id,
        task_type = task.task_type,
        attempt = task.attempts,
        reason,
        policy_version = POLICY,
        observed = evidence.observed,
        expected = evidence.expected,
        baseline_samples = evidence.baseline_samples,
        "Worker anomaly transition"
    );
    Ok(())
}
