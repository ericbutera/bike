use crate::{
    background_jobs::background_tasks,
    entities::{
        strava_delivery_intents as intents,
        strava_gateway::{Claim, Receipt},
    },
    jobs::JobQueue,
    workflow_error::WorkflowError,
};
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background_jobs::{
        entities::pipeline_runs, history_tests::database, pipeline::PipelineContext,
    };
    use sea_orm::{ConnectionTrait, PaginatorTrait, Schema};
    async fn fixture() -> DatabaseConnection {
        let db = database(true).await;
        let schema = Schema::new(db.get_database_backend());
        for statement in [
            schema.create_table_from_entity(intents::Entity),
            schema.create_table_from_entity(crate::entities::strava_connections::Entity),
        ] {
            db.execute(&statement).await.unwrap();
        }
        db
    }
    fn source() -> DeliverySource {
        DeliverySource {
            delivery_id: "rust:42".into(),
            athlete_id: 7,
            user_id: 7,
            activity_id: 42,
            event_time: 1,
            operation: "delete".into(),
            payload: None,
        }
    }
    #[tokio::test]
    async fn redelivery_preserves_receipt_and_enqueues_one_compact_task() {
        let db = fixture().await;
        let mut origin = PipelineContext::received("strava_webhook", Some("request".into()));
        origin.pipeline_started_at -= chrono::Duration::minutes(2);
        assert!(
            PipelineContext::scope(Some(origin.clone()), accept(&db, source()))
                .await
                .unwrap()
        );
        assert!(!accept(&db, source()).await.unwrap());
        assert_eq!(
            background_tasks::Entity::find().count(&db).await.unwrap(),
            1
        );
        let task = background_tasks::Entity::find()
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(task.task_type, "receive_strava_delivery");
        assert_eq!(
            task.payload["data"],
            serde_json::json!({"delivery_id":"rust:42"})
        );
        assert_eq!(
            pipeline_runs::Entity::find_by_id(origin.run_id)
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .pipeline_started_at,
            origin.pipeline_started_at
        );
        let mut changed = source();
        changed.activity_id = 43;
        assert!(accept(&db, changed).await.is_err());
        assert_eq!(intents::Entity::find().count(&db).await.unwrap(), 1);
    }
    #[tokio::test]
    async fn acceptance_rolls_back_source_when_the_task_intent_cannot_commit() {
        let db = fixture().await;
        db.execute_unprepared("DROP TABLE processing_pipeline_tasks")
            .await
            .unwrap();
        assert!(PipelineContext::scope(
            Some(PipelineContext::received("strava_webhook", None)),
            accept(&db, source())
        )
        .await
        .is_err());
        assert_eq!(intents::Entity::find().count(&db).await.unwrap(), 0);
        assert_eq!(
            background_tasks::Entity::find().count(&db).await.unwrap(),
            0
        );
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeliverySource {
    pub delivery_id: String,
    pub athlete_id: i64,
    pub user_id: i32,
    pub activity_id: i64,
    pub event_time: i64,
    pub operation: String,
    pub payload: Option<serde_json::Value>,
}
impl DeliverySource {
    pub fn receipt(&self) -> Receipt<'_> {
        Receipt {
            delivery_id: &self.delivery_id,
            athlete_id: self.athlete_id,
            user_id: self.user_id,
            activity_id: self.activity_id,
            event_time: self.event_time,
            operation: &self.operation,
        }
    }
}

/// Accepted source and compact task intent commit together. An ambiguous HTTP
/// response cannot replace the original receipt or enqueue another execution.
pub async fn accept(
    db: &DatabaseConnection,
    source: DeliverySource,
) -> Result<bool, WorkflowError> {
    super::validate_existing_connection(db, &source.receipt()).await?;
    let value = serde_json::to_value(&source)
        .map_err(|error| WorkflowError::internal(error.to_string()))?;
    let hash = format!("{:x}", Sha256::digest(value.to_string().as_bytes()));
    let txn = db.begin().await?;
    let inserted = intents::Entity::insert(intents::ActiveModel {
        delivery_id: Set(source.delivery_id.clone()),
        content_hash: Set(hash.clone()),
        source: Set(Some(value)),
        accepted_at: Set(chrono::Utc::now()),
        completed_at: Set(None),
    })
    .on_conflict(
        OnConflict::column(intents::Column::DeliveryId)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(&txn)
    .await?;
    if inserted > 0 {
        crate::background_jobs::entities::pipeline_outputs::Model::require(
            &txn,
            "gateway_delivery",
            source.user_id,
            source.delivery_id.clone(),
        )
        .await?;
        background_tasks::Model::enqueue(
            &txn,
            "receive_strava_delivery".into(),
            serde_json::json!({"data":{"delivery_id":source.delivery_id}}),
            None,
            5,
        )
        .await?;
    } else {
        let original = intents::Entity::find_by_id(&source.delivery_id)
            .one(&txn)
            .await?
            .ok_or_else(|| WorkflowError::internal("Accepted delivery disappeared"))?;
        if original.content_hash != hash {
            return Err(WorkflowError::conflict(
                "Delivery ID was reused with different source",
            ));
        }
    }
    txn.commit().await?;
    Ok(inserted > 0)
}

pub async fn execute(
    db: &DatabaseConnection,
    tasks: &JobQueue,
    uploads_dir: &str,
    id: &str,
) -> Result<(), WorkflowError> {
    let intent = intents::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Delivery intent not found"))?;
    if intent.completed_at.is_some() {
        return Ok(());
    }
    let source: DeliverySource = serde_json::from_value(
        intent
            .source
            .ok_or_else(|| WorkflowError::internal("Accepted source is missing"))?,
    )
    .map_err(|error| WorkflowError::internal(error.to_string()))?;
    let payload = source
        .payload
        .as_ref()
        .map(|payload| serde_json::from_value(payload.clone()))
        .transpose()
        .map_err(|error| WorkflowError::bad_request(error.to_string()))?;
    let claim = super::receive(db, tasks, uploads_dir, &source.receipt(), payload).await?;
    if claim == Claim::Busy {
        return Err(WorkflowError::conflict("Delivery execution lease is busy"));
    }
    let txn = db.begin().await?;
    let published_at = crate::entities::strava_gateway_receipts::Entity::find_by_id(id)
        .one(&txn)
        .await?
        .and_then(|receipt| receipt.completed_at)
        .ok_or_else(|| WorkflowError::internal("Delivery has no committed completion timestamp"))?;
    intents::Entity::update_many()
        .col_expr(
            intents::Column::Source,
            Expr::value(None::<serde_json::Value>),
        )
        .col_expr(intents::Column::CompletedAt, Expr::value(published_at))
        .filter(intents::Column::DeliveryId.eq(id))
        .exec(&txn)
        .await?;
    crate::background_jobs::entities::pipeline_outputs::Model::publish(
        &txn,
        "gateway_delivery",
        source.user_id,
        id,
        published_at,
    )
    .await?;
    txn.commit().await?;
    Ok(())
}
