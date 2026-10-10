//! Bounded diagnostics over the shared durable queue. Domain source data is never copied here.
mod anomalies;
mod graph;
mod health;
mod statistics;
#[cfg(test)]
mod statistics_tests;
pub use anomalies::completed;
pub use graph::{GraphCursor, PipelineGraph, PipelinePage};
pub use health::sample;
pub use statistics::{Distribution, ProcessorSummary};

use super::{
    entities::{background_tasks, pipeline_outputs, pipeline_runs, pipeline_tasks},
    worker::WorkerMetrics,
};
use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QuerySelect,
    TransactionTrait,
};

/// Reconcile only closed task trees with explicit, revision-matched output evidence.
/// CAS ensures a publication is counted once across replicas, using stored timestamps.
pub async fn reconcile_available(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
) -> Result<(), DbErr> {
    let outputs = pipeline_outputs::Entity::find()
        .select_only()
        .column(pipeline_outputs::Column::RunId)
        .into_query();
    let pending = pipeline_outputs::Entity::find()
        .select_only()
        .column(pipeline_outputs::Column::RunId)
        .filter(pipeline_outputs::Column::Required.eq(true))
        .filter(pipeline_outputs::Column::AvailableAt.is_null())
        .into_query();
    let unfinished = background_tasks::Entity::find()
        .select_only()
        .column(background_tasks::Column::Id)
        .filter(background_tasks::Column::Status.ne("completed"))
        .into_query();
    let open_trees = pipeline_tasks::Entity::find()
        .select_only()
        .column(pipeline_tasks::Column::RunId)
        .filter(pipeline_tasks::Column::TaskId.in_subquery(unfinished))
        .into_query();
    let candidates = pipeline_runs::Entity::find()
        .filter(pipeline_runs::Column::AvailableAt.is_null())
        .filter(pipeline_runs::Column::Id.in_subquery(outputs))
        .filter(pipeline_runs::Column::Id.not_in_subquery(pending))
        .filter(pipeline_runs::Column::Id.not_in_subquery(open_trees))
        .order_by_asc(pipeline_runs::Column::AcceptedAt)
        .limit(128)
        .all(db)
        .await?;
    for run in candidates {
        close_ready_run(db, metrics, run).await?;
    }
    Ok(())
}

async fn close_ready_run(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
    run: pipeline_runs::Model,
) -> Result<(), DbErr> {
    let transaction = db.begin().await?;
    let Some(run) = pipeline_runs::Entity::find_by_id(&run.id)
        .lock_exclusive()
        .one(&transaction)
        .await?
    else {
        return Ok(());
    };
    if run.available_at.is_some() {
        return Ok(());
    }
    let outputs = pipeline_outputs::Entity::find()
        .filter(pipeline_outputs::Column::RunId.eq(&run.id))
        .all(&transaction)
        .await?;
    if outputs.is_empty()
        || outputs
            .iter()
            .any(|output| output.required && output.available_at.is_none())
    {
        return Ok(());
    }
    let links = pipeline_tasks::Entity::find()
        .filter(pipeline_tasks::Column::RunId.eq(&run.id))
        .select_only()
        .column(pipeline_tasks::Column::TaskId)
        .into_tuple::<i32>()
        .all(&transaction)
        .await?;
    let unfinished = background_tasks::Entity::find()
        .select_only()
        .column(background_tasks::Column::Id)
        .filter(background_tasks::Column::Id.is_in(links))
        .filter(background_tasks::Column::Status.ne("completed"))
        .into_tuple::<i32>()
        .one(&transaction)
        .await?;
    if unfinished.is_some() {
        return Ok(());
    }
    let Some(available_at) = outputs
        .iter()
        .filter_map(|output| output.available_at)
        .max()
    else {
        return Ok(());
    };
    let changed = pipeline_runs::Entity::update_many()
        .col_expr(
            pipeline_runs::Column::AvailableAt,
            Expr::value(available_at),
        )
        .filter(pipeline_runs::Column::Id.eq(run.id))
        .filter(pipeline_runs::Column::AvailableAt.is_null())
        .exec(&transaction)
        .await?;
    if changed.rows_affected > 0 {
        transaction.commit().await?;
        metrics.record_available(
            &run.entrypoint,
            (available_at - run.pipeline_started_at).num_milliseconds() as f64 / 1000.0,
        );
    }
    Ok(())
}

use sea_orm::QueryOrder;

/// Closed diagnostics expire after 30 days. Accepted but unready runs, active
/// tasks and authoritative import artifacts/stages are deliberately retained.
pub async fn retain(db: &DatabaseConnection) -> Result<(), DbErr> {
    let cutoff = Utc::now() - chrono::Duration::days(30);
    prune_roots(db, cutoff).await?;
    prune_attempts(db, cutoff).await
}
async fn prune_roots(db: &DatabaseConnection, cutoff: chrono::DateTime<Utc>) -> Result<(), DbErr> {
    let roots = pipeline_runs::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(pipeline_runs::Column::AvailableAt.lt(cutoff))
                .add(
                    sea_orm::Condition::all()
                        .add(pipeline_runs::Column::AcceptedAt.lt(cutoff))
                        .add(
                            pipeline_runs::Column::Id.not_in_subquery(
                                pipeline_outputs::Entity::find()
                                    .select_only()
                                    .column(pipeline_outputs::Column::RunId)
                                    .filter(pipeline_outputs::Column::AvailableAt.is_null())
                                    .into_query(),
                            ),
                        )
                        .add(
                            pipeline_runs::Column::Id.not_in_subquery(
                                pipeline_tasks::Entity::find()
                                    .select_only()
                                    .column(pipeline_tasks::Column::RunId)
                                    .filter(
                                        pipeline_tasks::Column::TaskId.in_subquery(
                                            background_tasks::Entity::find()
                                                .select_only()
                                                .column(background_tasks::Column::Id)
                                                .filter(
                                                    background_tasks::Column::Status
                                                        .is_in(["pending", "processing"]),
                                                )
                                                .into_query(),
                                        ),
                                    )
                                    .into_query(),
                            ),
                        ),
                ),
        )
        .select_only()
        .column(pipeline_runs::Column::Id)
        .limit(128)
        .into_tuple::<String>()
        .all(db)
        .await?;
    let transaction = db.begin().await?;
    pipeline_tasks::Entity::delete_many()
        .filter(pipeline_tasks::Column::RunId.is_in(roots.clone()))
        .exec(&transaction)
        .await?;
    pipeline_outputs::Entity::delete_many()
        .filter(pipeline_outputs::Column::RunId.is_in(roots.clone()))
        .exec(&transaction)
        .await?;
    super::entities::pipeline_subjects::Entity::delete_many()
        .filter(super::entities::pipeline_subjects::Column::RunId.is_in(roots.clone()))
        .exec(&transaction)
        .await?;
    pipeline_runs::Entity::delete_many()
        .filter(pipeline_runs::Column::Id.is_in(roots))
        .exec(&transaction)
        .await?;
    transaction.commit().await
}
async fn prune_attempts(
    db: &DatabaseConnection,
    cutoff: chrono::DateTime<Utc>,
) -> Result<(), DbErr> {
    let closed = background_tasks::Entity::find()
        .select_only()
        .column(background_tasks::Column::Id)
        .filter(background_tasks::Column::CompletedAt.lt(cutoff))
        .filter(background_tasks::Column::Status.is_in(["completed", "canceled"]))
        .filter(
            background_tasks::Column::Id.in_subquery(
                super::entities::task_attempts::Entity::find()
                    .select_only()
                    .column(super::entities::task_attempts::Column::TaskId)
                    .into_query(),
            ),
        )
        .filter(
            background_tasks::Column::Id.not_in_subquery(
                pipeline_tasks::Entity::find()
                    .select_only()
                    .column(pipeline_tasks::Column::TaskId)
                    .into_query(),
            ),
        )
        .limit(128)
        .into_tuple::<i32>()
        .all(db)
        .await?;
    super::entities::work_units::Entity::delete_many()
        .filter(super::entities::work_units::Column::TaskId.is_in(closed.clone()))
        .exec(db)
        .await?;
    super::entities::task_attempts::Entity::delete_many()
        .filter(super::entities::task_attempts::Column::TaskId.is_in(closed.clone()))
        .exec(db)
        .await?;
    super::entities::task_anomalies::Entity::delete_many()
        .filter(super::entities::task_anomalies::Column::TaskId.is_in(closed))
        .exec(db)
        .await?;
    Ok(())
}

use sea_orm::QueryTrait;
#[cfg(test)]
mod tests;

pub async fn ready_count(db: &DatabaseConnection) -> Result<u64, DbErr> {
    use sea_orm::{Condition, ExprTrait, PaginatorTrait};
    background_tasks::Entity::find()
        .filter(background_tasks::Column::Status.eq("pending"))
        .filter(
            Expr::col(background_tasks::Column::Attempts)
                .lt(Expr::col(background_tasks::Column::MaxAttempts)),
        )
        .filter(
            Condition::any()
                .add(background_tasks::Column::ScheduledFor.is_null())
                .add(background_tasks::Column::ScheduledFor.lte(Utc::now())),
        )
        .count(db)
        .await
}
