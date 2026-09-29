use crate::workflow_error::WorkflowError as AppError;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};

pub struct Receipt<'a> {
    pub delivery_id: &'a str,
    pub athlete_id: i64,
    pub user_id: i32,
    pub activity_id: i64,
    pub event_time: i64,
    pub operation: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Claim {
    Acquired,
    Completed,
    Busy,
}

fn stmt(sql: &str, values: Vec<sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
}

pub async fn claim(db: &DatabaseConnection, receipt: &Receipt<'_>) -> Result<Claim, AppError> {
    let txn = db.begin().await?;
    txn.execute_raw(stmt(
        "INSERT INTO strava_gateway_bindings (athlete_id,user_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
        vec![receipt.athlete_id.into(), receipt.user_id.into()],
    ))
    .await?;

    let binding = txn
        .query_one_raw(stmt(
            "SELECT user_id,COALESCE(lease_until>now(),false) AS busy FROM strava_gateway_bindings WHERE athlete_id=$1 FOR UPDATE",
            vec![receipt.athlete_id.into()],
        ))
        .await?
        .ok_or_else(|| AppError::conflict("Strava athlete binding is unavailable"))?;
    let mapped_user: i32 = binding.try_get("", "user_id")?;
    if mapped_user != receipt.user_id {
        return Err(AppError::conflict(
            "Strava athlete is bound to another Bike user",
        ));
    }
    let busy: bool = binding.try_get("", "busy")?;
    if busy {
        return Ok(Claim::Busy);
    }

    if stale_receipt(&txn, receipt).await? {
        txn.execute_raw(stmt(
            "INSERT INTO strava_gateway_receipts (delivery_id,athlete_id,activity_id,event_time,operation,status,completed_at) VALUES ($1,$2,$3,$4,$5,'completed',now()) ON CONFLICT DO NOTHING",
            vec![receipt.delivery_id.into(), receipt.athlete_id.into(), receipt.activity_id.into(), receipt.event_time.into(), receipt.operation.into()],
        )).await?;
        txn.commit().await?;
        return Ok(Claim::Completed);
    }

    let inserted = txn
        .query_one_raw(stmt(
            "INSERT INTO strava_gateway_receipts (delivery_id,athlete_id,activity_id,event_time,operation,status,lease_until) VALUES ($1,$2,$3,$4,$5,'processing',now()+interval '2 minutes') ON CONFLICT (delivery_id) DO UPDATE SET lease_until=excluded.lease_until WHERE strava_gateway_receipts.status='processing' AND strava_gateway_receipts.lease_until<now() RETURNING status",
            vec![receipt.delivery_id.into(), receipt.athlete_id.into(), receipt.activity_id.into(), receipt.event_time.into(), receipt.operation.into()],
        )).await?;
    if inserted.is_some() {
        txn.execute_raw(stmt(
            "UPDATE strava_gateway_bindings SET active_delivery_id=$2,lease_until=now()+interval '2 minutes' WHERE athlete_id=$1",
            vec![receipt.athlete_id.into(), receipt.delivery_id.into()],
        ))
        .await?;
        txn.commit().await?;
        return Ok(Claim::Acquired);
    }
    let existing = txn
        .query_one_raw(stmt(
            "SELECT status FROM strava_gateway_receipts WHERE delivery_id=$1",
            vec![receipt.delivery_id.into()],
        ))
        .await?
        .ok_or_else(|| AppError::internal("Strava delivery receipt disappeared"))?;
    let status: String = existing.try_get("", "status")?;
    if status == "completed" {
        txn.commit().await?;
        Ok(Claim::Completed)
    } else {
        Ok(Claim::Busy)
    }
}

async fn stale_receipt(txn: &DatabaseTransaction, receipt: &Receipt<'_>) -> Result<bool, AppError> {
    let revoked = txn
        .query_one_raw(stmt(
            "SELECT event_time FROM strava_gateway_revocations WHERE athlete_id=$1",
            vec![receipt.athlete_id.into()],
        ))
        .await?;
    let revoked_at: Option<i64> = revoked
        .map(|row| row.try_get("", "event_time"))
        .transpose()?;
    let latest = txn
        .query_one_raw(stmt(
            "SELECT event_time,operation FROM strava_gateway_watermarks WHERE athlete_id=$1 AND activity_id=$2",
            vec![receipt.athlete_id.into(), receipt.activity_id.into()],
        ))
        .await?;
    let latest_event: Option<(i64, String)> = latest
        .map(|row| {
            Ok::<_, sea_orm::DbErr>((
                row.try_get("", "event_time")?,
                row.try_get("", "operation")?,
            ))
        })
        .transpose()?;
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
    let binding = txn
        .execute_raw(stmt(
            "UPDATE strava_gateway_bindings SET active_delivery_id=NULL,lease_until=NULL WHERE athlete_id=$1 AND active_delivery_id=$2",
            vec![receipt.athlete_id.into(), receipt.delivery_id.into()],
        ))
        .await?;
    if binding.rows_affected() != 1 {
        return Err(AppError::internal("Strava athlete delivery lease was lost"));
    }
    let result = txn
        .execute_raw(stmt(
            "UPDATE strava_gateway_receipts SET status='completed',lease_until=NULL,completed_at=now() WHERE delivery_id=$1 AND status='processing'",
            vec![receipt.delivery_id.into()],
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::internal("Strava delivery receipt lease was lost"));
    }
    txn.execute_raw(stmt(
        "INSERT INTO strava_gateway_watermarks (athlete_id,activity_id,event_time,operation) VALUES ($1,$2,$3,$4) ON CONFLICT (athlete_id,activity_id) DO UPDATE SET event_time=excluded.event_time,operation=excluded.operation WHERE strava_gateway_watermarks.event_time<=excluded.event_time",
        vec![receipt.athlete_id.into(), receipt.activity_id.into(), receipt.event_time.into(), receipt.operation.into()],
    )).await?;
    if receipt.operation == "deauthorize" {
        txn.execute_raw(stmt(
            "INSERT INTO strava_gateway_revocations (athlete_id,event_time) VALUES ($1,$2) ON CONFLICT (athlete_id) DO UPDATE SET event_time=GREATEST(strava_gateway_revocations.event_time,excluded.event_time)",
            vec![receipt.athlete_id.into(), receipt.event_time.into()],
        )).await?;
    }
    txn.commit().await?;
    Ok(())
}

pub async fn release(db: &DatabaseConnection, delivery_id: &str) -> Result<(), AppError> {
    let txn = db.begin().await?;
    txn.execute_raw(stmt(
        "UPDATE strava_gateway_bindings SET active_delivery_id=NULL,lease_until=NULL WHERE active_delivery_id=$1",
        vec![delivery_id.into()],
    ))
    .await?;
    txn.execute_raw(stmt(
        "UPDATE strava_gateway_receipts SET lease_until=now() WHERE delivery_id=$1 AND status='processing'",
        vec![delivery_id.into()],
    ))
    .await?;
    txn.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{claim, complete, stmt, Claim, Receipt};
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
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
        let row = db
            .query_one_raw(stmt(
                "INSERT INTO users (pid,email,api_key,name) VALUES (gen_random_uuid(),$1,$2,'Gateway test') RETURNING id",
                vec![format!("gateway-{unique}@example.invalid").into(), unique.clone().into()],
            ))
            .await
            .expect("create test user")
            .expect("created test user");
        let user_id: i32 = row.try_get("", "id").expect("user id");
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
        db.execute_raw(stmt(
            "DELETE FROM strava_gateway_receipts WHERE athlete_id=$1",
            vec![athlete_id.into()],
        ))
        .await
        .unwrap();
        db.execute_raw(stmt(
            "DELETE FROM strava_gateway_watermarks WHERE athlete_id=$1",
            vec![athlete_id.into()],
        ))
        .await
        .unwrap();
        db.execute_raw(stmt(
            "DELETE FROM strava_gateway_revocations WHERE athlete_id=$1",
            vec![athlete_id.into()],
        ))
        .await
        .unwrap();
        db.execute_raw(stmt(
            "DELETE FROM strava_gateway_bindings WHERE athlete_id=$1",
            vec![athlete_id.into()],
        ))
        .await
        .unwrap();
        db.execute_raw(stmt("DELETE FROM users WHERE id=$1", vec![user_id.into()]))
            .await
            .unwrap();
    }
}
