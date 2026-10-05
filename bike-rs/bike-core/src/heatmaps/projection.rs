//! Persistence for disposable heatmap projections. Source activity data stays authoritative.
use super::{geometry::Chunk, PROJECTION_VERSION};
use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement,
    TransactionTrait, Value,
};

#[derive(Debug, FromQueryResult)]
pub struct PendingProjection {
    pub activity_id: i32,
    pub generation: i64,
}

pub struct Projection;

impl Projection {
    pub async fn backfill_page<C: ConnectionTrait>(
        db: &C,
        user_id: i32,
        after_id: i32,
    ) -> Result<Vec<PendingProjection>, DbErr> {
        PendingProjection::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres,
            "SELECT activity_id,generation FROM heatmap_projections WHERE user_id=$1 AND activity_id>$2 AND status IN ('pending','failed') ORDER BY activity_id LIMIT 16",
            [user_id.into(),after_id.into()])).all(db).await
    }

    pub async fn enabled<C: ConnectionTrait>(db: &C) -> Result<bool, DbErr> {
        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT enabled FROM feature_flags WHERE feature_key='heatmaps'".to_owned(),
            ))
            .await?;
        row.map(|row| row.try_get("", "enabled"))
            .transpose()
            .map(|flag| flag.unwrap_or(false))
    }

    /// Queue and lease rows in the same transaction. Worker crashes and enqueue
    /// failures cannot strand durable dirty state; leases recover after 15 minutes.
    pub async fn enqueue_pending(db: &DatabaseConnection) -> Result<u64, DbErr> {
        let result = db.execute_raw(Statement::from_string(DbBackend::Postgres, r#"
            WITH candidates AS (
                SELECT activity_id FROM heatmap_projections
                WHERE status='pending' AND (queued_at IS NULL OR queued_at < now() - interval '15 minutes')
                ORDER BY activity_id LIMIT 16 FOR UPDATE SKIP LOCKED
            ), leased AS (
                UPDATE heatmap_projections p SET queued_at=now()
                FROM candidates c WHERE p.activity_id=c.activity_id
                RETURNING p.activity_id, p.generation
            )
            INSERT INTO background_tasks(task_type, payload, status, attempts, max_attempts, created_at, updated_at)
            SELECT 'prepare_heatmap',
                jsonb_build_object('type', 'PrepareHeatmap', 'data', jsonb_build_object('activities',
                    jsonb_agg(jsonb_build_object('activity_id', activity_id, 'generation', generation) ORDER BY activity_id))),
                'pending', 0, 3, now(), now()
            FROM leased
            HAVING count(*) > 0
        "#.to_owned())).await?;
        Ok(result.rows_affected())
    }

    pub async fn pending<C: ConnectionTrait>(
        db: &C,
        activity_id: i32,
        generation: i64,
    ) -> Result<bool, DbErr> {
        Ok(db.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "SELECT activity_id FROM heatmap_projections WHERE activity_id=$1 AND generation=$2 AND status IN ('pending','failed')",
            [activity_id.into(), generation.into()])).await?.is_some())
    }

    pub async fn publish(
        db: &DatabaseConnection,
        pending: &PendingProjection,
        chunks: &[Chunk],
    ) -> Result<bool, DbErr> {
        let txn = db.begin().await?;
        let locked = txn.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "SELECT user_id FROM heatmap_projections WHERE activity_id=$1 AND generation=$2 AND status IN ('pending','failed') FOR UPDATE",
            [pending.activity_id.into(), pending.generation.into()])).await?;
        let Some(locked) = locked else {
            txn.rollback().await?;
            return Ok(false);
        };
        let user_id: i32 = locked.try_get("", "user_id")?;
        txn.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM heatmap_chunks WHERE activity_id=$1",
            [pending.activity_id.into()],
        ))
        .await?;
        // Each insert is capped at 64 chunks / 256 KiB of coordinates.
        for (batch, group) in chunks.chunks(64).enumerate() {
            Self::insert_chunks(&txn, pending.activity_id, batch * 64, group).await?;
        }
        let bounds = chunks.iter().fold([1f64, 1f64, 0f64, 0f64], |b, c| {
            [
                b[0].min(c.bounds[0]),
                b[1].min(c.bounds[1]),
                b[2].max(c.bounds[2]),
                b[3].max(c.bounds[3]),
            ]
        });
        txn.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "UPDATE heatmap_projections SET status=$2, projection_version=$3, error=NULL, queued_at=NULL, min_x=$4,min_y=$5,max_x=$6,max_y=$7 WHERE activity_id=$1",
            vec![pending.activity_id.into(), if chunks.is_empty() { "skipped" } else { "ready" }.into(), PROJECTION_VERSION.into(),
                (!chunks.is_empty()).then_some(bounds[0]).into(), (!chunks.is_empty()).then_some(bounds[1]).into(), (!chunks.is_empty()).then_some(bounds[2]).into(), (!chunks.is_empty()).then_some(bounds[3]).into()])).await?;
        Self::bump_revision(&txn, user_id).await?;
        txn.commit().await?;
        Ok(true)
    }

    async fn insert_chunks<C: ConnectionTrait>(
        db: &C,
        activity_id: i32,
        offset: usize,
        chunks: &[Chunk],
    ) -> Result<(), DbErr> {
        let mut values: Vec<Value> = Vec::new();
        let mut rows = Vec::new();
        for (i, c) in chunks.iter().enumerate() {
            let n = values.len();
            rows.push(format!(
                "({})",
                (1..=8)
                    .map(|j| format!("${}", n + j))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            values.extend([
                activity_id.into(),
                c.band.into(),
                ((offset + i) as i32).into(),
                c.bounds[0].into(),
                c.bounds[1].into(),
                c.bounds[2].into(),
                c.bounds[3].into(),
                c.points.clone().into(),
            ]);
        }
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            format!("INSERT INTO heatmap_chunks(activity_id,band,chunk_index,min_x,min_y,max_x,max_y,points) VALUES {}",rows.join(",")), values)).await?;
        Ok(())
    }

    pub async fn record_failure<C: ConnectionTrait>(
        db: &C,
        pending: &PendingProjection,
    ) -> Result<(), DbErr> {
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "WITH changed AS (UPDATE heatmap_projections SET status='failed', error='Route preparation failed' WHERE activity_id=$1 AND generation=$2 AND status IN ('pending','failed') RETURNING user_id) UPDATE heatmap_user_states SET revision=revision+1 WHERE user_id IN (SELECT user_id FROM changed)",
            [pending.activity_id.into(), pending.generation.into()])).await?;
        Ok(())
    }

    async fn bump_revision<C: ConnectionTrait>(db: &C, user_id: i32) -> Result<(), DbErr> {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE heatmap_user_states SET revision=revision+1 WHERE user_id=$1",
            [user_id.into()],
        ))
        .await?;
        Ok(())
    }

    pub async fn source<C: ConnectionTrait>(
        db: &C,
        pending: &PendingProjection,
    ) -> Result<Option<ProjectionSource>, DbErr> {
        ProjectionSource::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres,
            "SELECT a.sport, a.source, a.title, a.derived_data_json, a.updated_at FROM activities a JOIN heatmap_projections p ON p.activity_id=a.id WHERE a.id=$1 AND p.generation=$2 AND p.status IN ('pending','failed')",
            [pending.activity_id.into(),pending.generation.into()])).one(db).await
    }
}

#[derive(FromQueryResult)]
pub struct ProjectionSource {
    pub sport: String,
    pub source: String,
    pub title: String,
    pub derived_data_json: Option<crate::activity_data::StoredActivityDerivedData>,
    pub updated_at: DateTime<Utc>,
}
