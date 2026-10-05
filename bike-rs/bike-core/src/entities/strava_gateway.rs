use super::{
    strava_gateway_bindings as bindings, strava_gateway_receipts as receipts,
    strava_gateway_revocations as revocations, strava_gateway_watermarks as watermarks,
};
use crate::integration_events_service::{self, NewIntegrationEvent, INTEGRATION_PROVIDER_STRAVA};
use crate::workflow_error::WorkflowError as AppError;
use sea_orm::sea_query::{Alias, Expr, OnConflict, Query, SimpleExpr};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    ExprTrait, QueryFilter, QuerySelect, Set, TransactionTrait,
};

pub struct Receipt<'a> {
    pub delivery_id: &'a str,
    pub athlete_id: i64,
    pub user_id: i32,
    pub activity_id: i64,
    pub event_time: i64,
    pub operation: &'a str,
}

impl Receipt<'_> {
    fn integration_event(
        &self,
        event_type: &str,
        level: &str,
        message: &str,
    ) -> NewIntegrationEvent {
        NewIntegrationEvent {
            user_id: Some(self.user_id),
            provider: INTEGRATION_PROVIDER_STRAVA.into(),
            event_type: event_type.into(),
            level: level.into(),
            message: message.into(),
            connection_id: None,
            payload: Some(serde_json::json!({
                "delivery_id": self.delivery_id, "athlete_id": self.athlete_id,
                "strava_activity_id": (self.operation != "deauthorize").then_some(self.activity_id),
                "event_time": self.event_time, "operation": self.operation,
            })),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Claim {
    Acquired,
    Completed,
    Busy,
}

pub async fn claim(db: &DatabaseConnection, receipt: &Receipt<'_>) -> Result<Claim, AppError> {
    let txn = db.begin().await?;
    create_binding(&txn, receipt).await?;
    let (mapped_user, busy) = bindings::Entity::find_by_id(receipt.athlete_id)
        .select_only()
        .column(bindings::Column::UserId)
        .expr_as(
            Expr::col(bindings::Column::LeaseUntil).gt(Expr::current_timestamp()),
            "busy",
        )
        .lock_exclusive()
        .into_tuple::<(i32, Option<bool>)>()
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::conflict("Strava athlete binding is unavailable"))?;
    if mapped_user != receipt.user_id {
        return Err(AppError::conflict(
            "Strava athlete is bound to another Bike user",
        ));
    }
    if busy.unwrap_or(false) {
        return Ok(Claim::Busy);
    }
    if stale_receipt(&txn, receipt).await? {
        complete_stale_receipt(&txn, receipt).await?;
        txn.commit().await?;
        return Ok(Claim::Completed);
    }
    if claim_processing_receipt(&txn, receipt).await? {
        mark_delivery_processing(&txn, receipt).await?;
        txn.commit().await?;
        return Ok(Claim::Acquired);
    }
    let existing = receipts::Entity::find_by_id(receipt.delivery_id)
        .select_only()
        .column(receipts::Column::Status)
        .into_tuple::<String>()
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::internal("Strava delivery receipt disappeared"))?;
    if existing == "completed" {
        txn.commit().await?;
        Ok(Claim::Completed)
    } else {
        Ok(Claim::Busy)
    }
}

async fn create_binding(txn: &DatabaseTransaction, receipt: &Receipt<'_>) -> Result<(), AppError> {
    bindings::Entity::insert(bindings::ActiveModel {
        athlete_id: Set(receipt.athlete_id),
        user_id: Set(receipt.user_id),
        ..Default::default()
    })
    .on_conflict(OnConflict::new().do_nothing().to_owned())
    .try_insert()
    .exec(txn)
    .await?;
    Ok(())
}

fn lease_deadline() -> SimpleExpr {
    // PostgreSQL interval arithmetic keeps lease decisions on the database clock.
    Expr::current_timestamp().add(Expr::val("2 minutes").cast_as("interval"))
}

async fn claim_processing_receipt(
    txn: &DatabaseTransaction,
    receipt: &Receipt<'_>,
) -> Result<bool, AppError> {
    // SeaQuery expresses the atomic conditional upsert and database-clock lease;
    // ActiveModel values alone cannot represent an INSERT timestamp expression.
    let query = Query::insert()
        .into_table(receipts::Entity)
        .columns([
            receipts::Column::DeliveryId,
            receipts::Column::AthleteId,
            receipts::Column::ActivityId,
            receipts::Column::EventTime,
            receipts::Column::Operation,
            receipts::Column::Status,
            receipts::Column::LeaseUntil,
        ])
        .values_panic([
            Expr::val(receipt.delivery_id),
            Expr::val(receipt.athlete_id),
            Expr::val(receipt.activity_id),
            Expr::val(receipt.event_time),
            Expr::val(receipt.operation),
            Expr::val("processing"),
            lease_deadline(),
        ])
        .on_conflict(
            OnConflict::column(receipts::Column::DeliveryId)
                .update_column(receipts::Column::LeaseUntil)
                .action_cond_where(
                    Condition::all()
                        .add(
                            Expr::col((receipts::Entity, receipts::Column::Status))
                                .eq("processing"),
                        )
                        .add(
                            Expr::col((receipts::Entity, receipts::Column::LeaseUntil))
                                .lt(Expr::current_timestamp()),
                        ),
                )
                .to_owned(),
        )
        .returning_col(receipts::Column::DeliveryId)
        .to_owned();
    Ok(txn.query_one(&query).await?.is_some())
}

async fn mark_delivery_processing(
    txn: &DatabaseTransaction,
    receipt: &Receipt<'_>,
) -> Result<(), AppError> {
    bindings::Entity::update_many()
        .col_expr(
            bindings::Column::ActiveDeliveryId,
            Expr::val(receipt.delivery_id),
        )
        .col_expr(bindings::Column::LeaseUntil, lease_deadline())
        .filter(bindings::Column::AthleteId.eq(receipt.athlete_id))
        .exec(txn)
        .await?;
    integration_events_service::record_event(
        txn,
        receipt.integration_event(
            "gateway.delivery.received",
            "info",
            "Received a Strava gateway delivery for processing.",
        ),
    )
    .await?;
    Ok(())
}

async fn complete_stale_receipt(
    txn: &DatabaseTransaction,
    receipt: &Receipt<'_>,
) -> Result<(), AppError> {
    // Use an INSERT expression to preserve the database-clock completion time.
    let query = Query::insert()
        .into_table(receipts::Entity)
        .columns([
            receipts::Column::DeliveryId,
            receipts::Column::AthleteId,
            receipts::Column::ActivityId,
            receipts::Column::EventTime,
            receipts::Column::Operation,
            receipts::Column::Status,
            receipts::Column::CompletedAt,
        ])
        .values_panic([
            Expr::val(receipt.delivery_id),
            Expr::val(receipt.athlete_id),
            Expr::val(receipt.activity_id),
            Expr::val(receipt.event_time),
            Expr::val(receipt.operation),
            Expr::val("completed"),
            Expr::current_timestamp(),
        ])
        .on_conflict(OnConflict::new().do_nothing().to_owned())
        .returning_col(receipts::Column::DeliveryId)
        .to_owned();
    if txn.query_one(&query).await?.is_some() {
        integration_events_service::record_event(
            txn,
            receipt.integration_event(
                "gateway.delivery.ignored",
                "warning",
                "Ignored an older Strava gateway delivery.",
            ),
        )
        .await?;
    }
    Ok(())
}

async fn stale_receipt(txn: &DatabaseTransaction, receipt: &Receipt<'_>) -> Result<bool, AppError> {
    let revoked_at = revocations::Entity::find_by_id(receipt.athlete_id)
        .select_only()
        .column(revocations::Column::EventTime)
        .into_tuple::<i64>()
        .one(txn)
        .await?;
    let latest_event = watermarks::Entity::find_by_id((receipt.athlete_id, receipt.activity_id))
        .select_only()
        .columns([watermarks::Column::EventTime, watermarks::Column::Operation])
        .into_tuple::<(i64, String)>()
        .one(txn)
        .await?;
    Ok(latest_event.is_some_and(|(event_time, operation)| {
        event_time > receipt.event_time
            || (event_time == receipt.event_time
                && operation == "delete"
                && receipt.operation == "upsert")
    }) || (receipt.operation == "upsert"
        && revoked_at.is_some_and(|event_time| event_time >= receipt.event_time)))
}

pub async fn complete(db: &DatabaseConnection, receipt: &Receipt<'_>) -> Result<(), AppError> {
    let txn = db.begin().await?;
    let binding = bindings::Entity::update_many()
        .set(bindings::ActiveModel {
            active_delivery_id: Set(None),
            lease_until: Set(None),
            ..Default::default()
        })
        .filter(bindings::Column::AthleteId.eq(receipt.athlete_id))
        .filter(bindings::Column::ActiveDeliveryId.eq(receipt.delivery_id))
        .exec(&txn)
        .await?;
    if binding.rows_affected != 1 {
        return Err(AppError::internal("Strava athlete delivery lease was lost"));
    }
    let result = receipts::Entity::update_many()
        .set(receipts::ActiveModel {
            status: Set("completed".into()),
            lease_until: Set(None),
            ..Default::default()
        })
        .col_expr(receipts::Column::CompletedAt, Expr::current_timestamp())
        .filter(receipts::Column::DeliveryId.eq(receipt.delivery_id))
        .filter(receipts::Column::Status.eq("processing"))
        .exec(&txn)
        .await?;
    if result.rows_affected != 1 {
        return Err(AppError::internal("Strava delivery receipt lease was lost"));
    }
    advance_watermark(&txn, receipt).await?;
    if receipt.operation == "deauthorize" {
        advance_revocation(&txn, receipt).await?;
    }
    integration_events_service::record_event(
        &txn,
        receipt.integration_event(
            "gateway.delivery.applied",
            "success",
            &format!("Applied Strava gateway {} delivery.", receipt.operation),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

async fn advance_watermark(
    txn: &DatabaseTransaction,
    receipt: &Receipt<'_>,
) -> Result<(), AppError> {
    watermarks::Entity::insert(watermarks::ActiveModel {
        athlete_id: Set(receipt.athlete_id),
        activity_id: Set(receipt.activity_id),
        event_time: Set(receipt.event_time),
        operation: Set(receipt.operation.into()),
    })
    .on_conflict(
        OnConflict::columns([
            watermarks::Column::AthleteId,
            watermarks::Column::ActivityId,
        ])
        .update_columns([watermarks::Column::EventTime, watermarks::Column::Operation])
        .action_cond_where(
            Expr::col((watermarks::Entity, watermarks::Column::EventTime)).lte(Expr::col((
                Alias::new("excluded"),
                watermarks::Column::EventTime,
            ))),
        )
        .to_owned(),
    )
    .try_insert()
    .exec(txn)
    .await?;
    Ok(())
}

async fn advance_revocation(
    txn: &DatabaseTransaction,
    receipt: &Receipt<'_>,
) -> Result<(), AppError> {
    revocations::Entity::insert(revocations::ActiveModel {
        athlete_id: Set(receipt.athlete_id),
        event_time: Set(receipt.event_time),
    })
    .on_conflict(
        OnConflict::column(revocations::Column::AthleteId)
            .update_column(revocations::Column::EventTime)
            .action_cond_where(
                Expr::col((revocations::Entity, revocations::Column::EventTime)).lte(Expr::col((
                    Alias::new("excluded"),
                    revocations::Column::EventTime,
                ))),
            )
            .to_owned(),
    )
    .try_insert()
    .exec(txn)
    .await?;
    Ok(())
}

pub async fn release(
    db: &DatabaseConnection,
    receipt: &Receipt<'_>,
    error: &AppError,
) -> Result<(), AppError> {
    let txn = db.begin().await?;
    bindings::Entity::update_many()
        .set(bindings::ActiveModel {
            active_delivery_id: Set(None),
            lease_until: Set(None),
            ..Default::default()
        })
        .filter(bindings::Column::ActiveDeliveryId.eq(receipt.delivery_id))
        .exec(&txn)
        .await?;
    receipts::Entity::update_many()
        .col_expr(receipts::Column::LeaseUntil, Expr::current_timestamp())
        .filter(receipts::Column::DeliveryId.eq(receipt.delivery_id))
        .filter(receipts::Column::Status.eq("processing"))
        .exec(&txn)
        .await?;
    let mut event = receipt.integration_event("gateway.delivery.failed", "error", &error.message);
    event.payload.as_mut().expect("receipt event payload")["status_code"] =
        error.status.as_u16().into();
    integration_events_service::record_event(&txn, event).await?;
    txn.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bindings, claim, complete, receipts, revocations, watermarks, Claim, Receipt};
    use crate::auth::entities::users;
    use crate::entities::integration_events;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter, Set,
    };
    use uuid::Uuid;

    #[tokio::test]
    async fn receipts_are_idempotent_and_revocation_blocks_older_upserts() {
        let Ok(url) = std::env::var("BIKE_TEST_DATABASE_URL") else {
            return;
        };
        let db = Database::connect(&url)
            .await
            .expect("connect to migrated test database");
        let unique = Uuid::new_v4().to_string();
        let user = users::ActiveModel {
            pid: Set(Uuid::new_v4()),
            email: Set(format!("gateway-{unique}@example.invalid")),
            api_key: Set(unique.clone()),
            name: Set("Gateway test".into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("create test user");
        let user_id = user.id;
        let athlete_id = i64::from(user_id) + 9_000_000_000;
        let first_id = format!("rust:{unique}:1");
        let first = Receipt {
            delivery_id: &first_id,
            athlete_id,
            user_id,
            activity_id: 42,
            event_time: 100,
            operation: "delete",
        };
        assert_eq!(claim(&db, &first).await.unwrap(), Claim::Acquired);
        assert_eq!(claim(&db, &first).await.unwrap(), Claim::Busy);
        let second_id = format!("rust:{unique}:concurrent");
        let concurrent = Receipt {
            delivery_id: &second_id,
            event_time: 101,
            ..first
        };
        assert_eq!(claim(&db, &concurrent).await.unwrap(), Claim::Busy);
        complete(&db, &first).await.unwrap();
        assert_eq!(claim(&db, &first).await.unwrap(), Claim::Completed);
        assert_eq!(claim(&db, &concurrent).await.unwrap(), Claim::Acquired);
        complete(&db, &concurrent).await.unwrap();
        let equal_id = format!("rust:{unique}:equal");
        let equal_upsert = Receipt {
            delivery_id: &equal_id,
            operation: "upsert",
            ..first
        };
        assert_eq!(claim(&db, &equal_upsert).await.unwrap(), Claim::Completed);

        let deauth_id = format!("rust:{unique}:2");
        let deauth = Receipt {
            delivery_id: &deauth_id,
            activity_id: athlete_id,
            event_time: 200,
            operation: "deauthorize",
            ..first
        };
        assert_eq!(claim(&db, &deauth).await.unwrap(), Claim::Acquired);
        complete(&db, &deauth).await.unwrap();

        let old_id = format!("rust:{unique}:3");
        let old_upsert = Receipt {
            delivery_id: &old_id,
            event_time: 150,
            operation: "upsert",
            ..first
        };
        assert_eq!(claim(&db, &old_upsert).await.unwrap(), Claim::Completed);
        let fresh_id = format!("rust:{unique}:4");
        let fresh_upsert = Receipt {
            delivery_id: &fresh_id,
            event_time: 201,
            operation: "upsert",
            ..first
        };
        assert_eq!(claim(&db, &fresh_upsert).await.unwrap(), Claim::Acquired);

        cleanup(&db, athlete_id, user_id).await;
    }

    async fn cleanup(db: &DatabaseConnection, athlete_id: i64, user_id: i32) {
        integration_events::Entity::delete_many()
            .filter(integration_events::Column::UserId.eq(user_id))
            .exec(db)
            .await
            .unwrap();
        receipts::Entity::delete_many()
            .filter(receipts::Column::AthleteId.eq(athlete_id))
            .exec(db)
            .await
            .unwrap();
        watermarks::Entity::delete_many()
            .filter(watermarks::Column::AthleteId.eq(athlete_id))
            .exec(db)
            .await
            .unwrap();
        revocations::Entity::delete_many()
            .filter(revocations::Column::AthleteId.eq(athlete_id))
            .exec(db)
            .await
            .unwrap();
        bindings::Entity::delete_by_id(athlete_id)
            .exec(db)
            .await
            .unwrap();
        users::Entity::delete_by_id(user_id).exec(db).await.unwrap();
    }
}
