//! Persistence for disposable heatmap projections. Source activity data stays authoritative.
use crate::entities::{
    activities, heatmap_chunks as chunks_entity, heatmap_projections as projections, heatmap_user_states as user_states,
};
use crate::heatmaps::{geometry::Chunk, PROJECTION_VERSION};
use crate::platform::feature_flags::entities as feature_flags;
use chrono::{DateTime, Utc};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, ExprTrait, FromQueryResult,
    JoinType, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set, Statement, TransactionTrait,
};

#[derive(Debug, FromQueryResult)]
pub struct PendingProjection {
    pub activity_id: i32,
    pub generation: i64,
}

pub struct Projection;

impl Projection {
    pub async fn pending_tasks(
        db: &impl ConnectionTrait,
        after_id: i32,
    ) -> Result<Vec<crate::bike_tasks::ActivityTask>, DbErr> {
        if !Self::enabled(db).await? {
            return Ok(Vec::new());
        }
        let rows = projections::Entity::find()
            .filter(projections::Column::ActivityId.gt(after_id))
            .filter(projections::Column::Status.is_in(["pending", "failed"]))
            .filter(projections::Column::ProjectionVersion.eq(PROJECTION_VERSION))
            .order_by_asc(projections::Column::ActivityId)
            .limit(16)
            .all(db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| crate::bike_tasks::ActivityTask {
                activity_id: row.activity_id,
                user_id: row.user_id,
                priority: crate::bike_tasks::Priority::Background,
            })
            .collect())
    }
    /// Invalidate an owned route within the caller's source-change transaction.
    pub async fn invalidate_owned(db: &impl ConnectionTrait, activity_id: i32, user_id: i32) -> Result<(), DbErr> {
        let changed = projections::Entity::update_many()
            .set(projections::ActiveModel {
                status: Set("pending".into()),
                projection_version: Set(PROJECTION_VERSION),
                queued_at: Set(None),
                published_at: Set(None),
                error: Set(None),
                min_x: Set(None),
                min_y: Set(None),
                max_x: Set(None),
                max_y: Set(None),
                ..Default::default()
            })
            .col_expr(
                projections::Column::Generation,
                Expr::col(projections::Column::Generation).add(1),
            )
            .filter(projections::Column::ActivityId.eq(activity_id))
            .filter(projections::Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        if changed.rows_affected > 0 {
            chunks_entity::Model::clear_activity(db, activity_id).await?;
            user_states::Model::bump_revision(db, user_id).await?;
        }
        Ok(())
    }

    pub async fn backfill_page(
        db: &impl ConnectionTrait,
        user_id: i32,
        after_id: i32,
    ) -> Result<Vec<PendingProjection>, DbErr> {
        projections::Entity::find()
            .select_only()
            .columns([projections::Column::ActivityId, projections::Column::Generation])
            .filter(projections::Column::UserId.eq(user_id))
            .filter(projections::Column::ActivityId.gt(after_id))
            .filter(projections::Column::Status.is_in(["pending", "failed"]))
            .filter(projections::Column::ProjectionVersion.eq(PROJECTION_VERSION))
            .order_by_asc(projections::Column::ActivityId)
            .limit(16)
            .into_model::<PendingProjection>()
            .all(db)
            .await
    }

    pub async fn enabled(db: &impl ConnectionTrait) -> Result<bool, DbErr> {
        feature_flags::Model::enabled(db, "heatmaps").await
    }

    pub fn revision(generation: i64) -> String {
        format!("{generation}:{PROJECTION_VERSION}")
    }

    pub async fn pending(db: &impl ConnectionTrait, activity_id: i32, generation: i64) -> Result<bool, DbErr> {
        Ok(projections::Entity::find_by_id(activity_id)
            .select_only()
            .column(projections::Column::ActivityId)
            .filter(projections::Column::Generation.eq(generation))
            .filter(projections::Column::ProjectionVersion.eq(PROJECTION_VERSION))
            .filter(projections::Column::Status.is_in(["pending", "failed"]))
            .into_tuple::<i32>()
            .one(db)
            .await?
            .is_some())
    }

    pub async fn publish(
        db: &DatabaseConnection,
        pending: &PendingProjection,
        chunks: &[Chunk],
    ) -> Result<bool, DbErr> {
        let txn = db.begin().await?;
        let locked = projections::Entity::find_by_id(pending.activity_id)
            .select_only()
            .column(projections::Column::UserId)
            .filter(projections::Column::Generation.eq(pending.generation))
            .filter(projections::Column::ProjectionVersion.eq(PROJECTION_VERSION))
            .filter(projections::Column::Status.is_in(["pending", "failed"]))
            .lock_exclusive()
            .into_tuple::<i32>()
            .one(&txn)
            .await?;
        let Some(user_id) = locked else {
            txn.rollback().await?;
            return Ok(false);
        };
        chunks_entity::Model::clear_activity(&txn, pending.activity_id).await?;
        // Each insert is capped at 64 chunks / 256 KiB of coordinates.
        for (batch, group) in chunks.chunks(64).enumerate() {
            chunks_entity::Model::insert_chunks(&txn, pending.activity_id, batch * 64, group).await?;
        }
        let bounds = chunks.iter().fold([1f64, 1f64, 0f64, 0f64], |b, c| {
            [
                b[0].min(c.bounds[0]),
                b[1].min(c.bounds[1]),
                b[2].max(c.bounds[2]),
                b[3].max(c.bounds[3]),
            ]
        });
        let published_at = Utc::now();
        projections::Entity::update_many()
            .set(projections::ActiveModel {
                status: Set(if chunks.is_empty() { "skipped" } else { "ready" }.into()),
                projection_version: Set(PROJECTION_VERSION),
                error: Set(None),
                queued_at: Set(None),
                published_at: Set(Some(published_at)),
                min_x: Set((!chunks.is_empty()).then_some(bounds[0])),
                min_y: Set((!chunks.is_empty()).then_some(bounds[1])),
                max_x: Set((!chunks.is_empty()).then_some(bounds[2])),
                max_y: Set((!chunks.is_empty()).then_some(bounds[3])),
                ..Default::default()
            })
            .filter(projections::Column::ActivityId.eq(pending.activity_id))
            .exec(&txn)
            .await?;
        user_states::Model::bump_revision(&txn, user_id).await?;
        txn.commit().await?;
        Ok(true)
    }

    pub async fn record_failure(db: &impl ConnectionTrait, pending: &PendingProjection) -> Result<(), DbErr> {
        // One guarded writable CTE updates the failed generation and its owner
        // revision together; a newer projection must not invalidate the cache.
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "WITH changed AS (UPDATE heatmap_projections SET status='failed', error='Route preparation failed' WHERE activity_id=$1 AND generation=$2 AND status IN ('pending','failed') RETURNING user_id) UPDATE heatmap_user_states SET revision=revision+1 WHERE user_id IN (SELECT user_id FROM changed)",
            [pending.activity_id.into(), pending.generation.into()])).await?;
        Ok(())
    }

    pub async fn source(
        db: &impl ConnectionTrait,
        pending: &PendingProjection,
    ) -> Result<Option<ProjectionSource>, DbErr> {
        projections::Entity::find()
            .select_only()
            .columns([
                activities::Column::Sport,
                activities::Column::Format,
                activities::Column::Source,
                activities::Column::Title,
                activities::Column::DerivedDataJson,
                activities::Column::UpdatedAt,
                activities::Column::UserId,
                activities::Column::StartedAt,
            ])
            .join(JoinType::InnerJoin, projections::Relation::Activities.def())
            .filter(projections::Column::ActivityId.eq(pending.activity_id))
            .filter(projections::Column::Generation.eq(pending.generation))
            .filter(projections::Column::ProjectionVersion.eq(PROJECTION_VERSION))
            .filter(projections::Column::Status.is_in(["pending", "failed"]))
            .into_model::<ProjectionSource>()
            .one(db)
            .await
    }
}

#[derive(FromQueryResult)]
pub struct ProjectionSource {
    pub user_id: i32,
    pub started_at: DateTime<Utc>,
    pub sport: String,
    pub format: Option<String>,
    pub source: String,
    pub title: String,
    pub derived_data_json: Option<crate::activity_data::StoredActivityDerivedData>,
    pub updated_at: DateTime<Utc>,
}
