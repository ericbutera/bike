use super::{stage::json_error, Source};
use crate::{
    background_jobs::background_tasks,
    entities::{integration_events, strava_connections, strava_workflows},
    strava::StravaWebhookEvent,
    strava_objects::{checksum, ObjectStore},
    workflow_error::WorkflowError,
};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ConnectionTrait, EntityTrait, Set, TransactionTrait,
};

pub async fn accept_webhook(
    db: &sea_orm::DatabaseConnection,
    event: &StravaWebhookEvent,
    raw: &[u8],
) -> Result<bool, WorkflowError> {
    if let Ok(expected) = std::env::var("STRAVA_SUBSCRIPTION_ID") {
        if expected
            .parse::<i64>()
            .map_err(|_| WorkflowError::internal("Invalid Strava subscription ID"))?
            != event.subscription_id
        {
            return Err(WorkflowError::bad_request(
                "Strava subscription ID does not match",
            ));
        }
    }
    let Some(connection) = strava_connections::Model::for_athlete(db, event.owner_id).await? else {
        return Ok(false);
    };
    let Some(action) = webhook_action(event) else {
        return Ok(false);
    };
    let store = ObjectStore::new(&crate::config::Config::get().uploads_dir);
    let raw = store
        .write(
            &format!(
                "strava/{}/{}/raw/{}/webhook",
                event.owner_id, event.object_id, event.event_time
            ),
            raw,
        )
        .await?;
    let id = format!(
        "webhook-{}-{}-{}-{}-{}",
        connection.created_at.timestamp_micros(),
        event.owner_id,
        event.object_id,
        event.event_time,
        checksum(&serde_json::to_vec(event).map_err(json_error)?)
    );
    let source = Source {
        revoke_remote: false,
        generation: connection.created_at,
        event_time: event.event_time,
        parent: None,
        activity: None,
        listing_page: None,
        raw: Some(raw),
        after: None,
    };
    let txn = db.begin().await?;
    let inserted = accept(
        &txn,
        &connection,
        &id,
        action,
        (event.object_type == "activity").then_some(event.object_id),
        source,
    )
    .await?;
    if inserted {
        enqueue(&txn, &id).await?;
    }
    txn.commit().await?;
    Ok(inserted)
}

pub async fn queue_sync(
    db: &sea_orm::DatabaseConnection,
    connection: &strava_connections::Model,
    backfill: bool,
) -> Result<String, WorkflowError> {
    if strava_workflows::Model::pending_revocation(db, connection)
        .await?
        .is_some()
    {
        return Err(WorkflowError::conflict("Strava disconnect is pending"));
    }
    let txn = db.begin().await?;
    if db.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
        // Serialize acceptance, separately from the provider execution lease.
        txn.execute_unprepared(&format!(
            "SELECT pg_advisory_xact_lock(7392, {})",
            connection.id
        ))
        .await?;
    }
    if let Some(active) = strava_workflows::Model::active_collection(&txn, connection.id).await? {
        txn.commit().await?;
        return Ok(active.id);
    }
    let id = uuid::Uuid::new_v4().to_string();
    let accepted_at = chrono::Utc::now();
    let after = collection_after(
        connection.last_synced_activity_started_at,
        backfill,
        accepted_at,
    );
    let source = Source {
        revoke_remote: false,
        generation: connection.created_at,
        event_time: accepted_at.timestamp(),
        parent: None,
        activity: None,
        listing_page: None,
        raw: None,
        after: Some(after),
    };
    accept(
        &txn,
        connection,
        &id,
        if backfill { "backfill" } else { "sync" },
        None,
        source,
    )
    .await?;
    enqueue(&txn, &id).await?;
    let mut queued: strava_connections::ActiveModel = connection.clone().into();
    queued.last_sync_status = Set("queued".into());
    queued.last_sync_message = Set(Some(format!(
        "Strava {} workflow {id} queued.",
        if backfill { "backfill" } else { "sync" }
    )));
    queued.update(&txn).await?;
    txn.commit().await?;
    Ok(id)
}

pub async fn queue_disconnect(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
) -> Result<Option<String>, WorkflowError> {
    let Some(connection) = crate::strava::load_connection(db, user_id).await? else {
        return Ok(None);
    };
    let id = format!(
        "disconnect-{}-{}",
        connection.athlete_id,
        connection.created_at.timestamp_micros()
    );
    let source = Source {
        revoke_remote: true,
        generation: connection.created_at,
        event_time: chrono::Utc::now().timestamp(),
        parent: None,
        activity: None,
        listing_page: None,
        raw: None,
        after: None,
    };
    let txn = db.begin().await?;
    if accept(&txn, &connection, &id, "deauthorize", None, source).await? {
        enqueue(&txn, &id).await?;
    }
    txn.commit().await?;
    Ok(Some(id))
}

pub async fn queue_reconciliation(
    db: &sea_orm::DatabaseConnection,
) -> Result<usize, WorkflowError> {
    if !super::provider_work_enabled() {
        return Ok(0);
    }
    let mut queued = 0;
    for connection in strava_connections::Model::all_connected(db).await? {
        if strava_workflows::Model::pending_revocation(db, &connection)
            .await?
            .is_none()
        {
            queue_sync(db, &connection, false).await?;
            queued += 1;
        }
    }
    Ok(queued)
}

pub(super) async fn accept(
    db: &impl sea_orm::ConnectionTrait,
    connection: &strava_connections::Model,
    id: &str,
    action: &str,
    activity_id: Option<i64>,
    source: Source,
) -> Result<bool, WorkflowError> {
    let result = strava_workflows::Entity::insert(strava_workflows::ActiveModel {
        id: Set(id.into()),
        connection_id: Set(connection.id),
        athlete_id: Set(connection.athlete_id),
        activity_id: Set(activity_id),
        action: Set(action.into()),
        source: Set(serde_json::to_value(&source).map_err(json_error)?),
        checkpoint: Set(serde_json::json!({})),
        status: Set("accepted".into()),
        error: Set(None),
        accepted_at: Set(chrono::Utc::now()),
        completed_at: Set(None),
    })
    .on_conflict(
        OnConflict::column(strava_workflows::Column::Id)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
    if result > 0 {
        integration_events::Entity::record(db, integration_events::NewEvent {
            user_id: Some(connection.user_id),
            connection_id: Some(connection.id),
            provider: "strava".into(),
            event_type: "workflow.accepted".into(),
            level: "info".into(),
            message: format!("Accepted Strava {action} workflow."),
            payload: Some(serde_json::json!({"workflow_id":id,"correlation_id":activity_id.map(|id| id.to_string()).unwrap_or_else(|| id.into()),"strava_activity_id":activity_id,"athlete_id":connection.athlete_id,"action":action,"source":source,"stages":super::graph(action)?})),
        }).await?;
        tracing::info!(
            correlation_id = %activity_id.map(|id| id.to_string()).unwrap_or_else(|| id.into()),
            workflow_id = id,
            strava_activity_id = activity_id,
            athlete_id = connection.athlete_id,
            action,
            "Accepted Strava workflow"
        );
    }
    Ok(result > 0)
}
async fn enqueue(
    db: &(impl sea_orm::ConnectionTrait + TransactionTrait),
    id: &str,
) -> Result<(), WorkflowError> {
    let row = strava_workflows::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Strava workflow was not found"))?;
    background_tasks::Model::enqueue(db, "strava_workflow".into(), job_payload(&row), None, 12)
        .await?;
    Ok(())
}

pub async fn resume_at(
    db: &sea_orm::DatabaseConnection,
    id: &str,
    at: chrono::DateTime<chrono::Utc>,
) -> Result<(), WorkflowError> {
    let txn = db.begin().await?;
    let row = strava_workflows::Entity::find_by_id(id)
        .one(&txn)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Strava workflow was not found"))?;
    {
        use sea_orm::ActiveModelTrait;
        let mut active: strava_workflows::ActiveModel = row.clone().into();
        let mut checkpoint = row.checkpoint.clone();
        checkpoint["pause"] = serde_json::json!({"retry_at":at,"reason":if super::provider_work_enabled() {"rate_limit"} else {"provider_paused"}});
        active.checkpoint = Set(checkpoint);
        active.status = Set("accepted".into());
        active.update(&txn).await?;
    }
    background_tasks::Model::enqueue(
        &txn,
        "strava_workflow".into(),
        job_payload(
            &strava_workflows::Entity::find_by_id(id)
                .one(&txn)
                .await?
                .ok_or_else(|| WorkflowError::not_found("Strava workflow was not found"))?,
        ),
        Some(at),
        12,
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

fn job_payload(row: &strava_workflows::Model) -> serde_json::Value {
    serde_json::json!({"data":{"workflow_id":row.id,"strava_activity_id":row.activity_id,"athlete_id":row.athlete_id,"action":row.action},"strava":row.worker_context()})
}

pub async fn queue_user_sync(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    backfill: bool,
) -> Result<strava_connections::Model, WorkflowError> {
    let connection = crate::strava::load_connection(db, user_id)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Connect Strava before starting a sync"))?;
    crate::strava::ensure_connection_scopes_allow_activity_import(&connection)?;
    queue_sync(db, &connection, backfill).await?;
    crate::strava::load_connection(db, user_id)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Strava connection was removed"))
}

pub async fn active_user_collection(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
) -> Result<Option<strava_workflows::Model>, WorkflowError> {
    let Some(connection) = crate::strava::load_connection(db, user_id).await? else {
        return Ok(None);
    };
    Ok(strava_workflows::Model::active_collection(db, connection.id).await?)
}

fn collection_after(
    cursor: Option<chrono::DateTime<chrono::Utc>>,
    backfill: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> i64 {
    let initial = now - chrono::Duration::days(super::INITIAL_BACKFILL_DAYS);
    if backfill {
        initial
    } else {
        cursor.unwrap_or(initial)
    }
    .timestamp()
}

fn webhook_action(event: &StravaWebhookEvent) -> Option<&'static str> {
    match (event.object_type.as_str(), event.aspect_type.as_str()) {
        ("activity", "create") => Some("insert"),
        ("activity", "update") => Some("update"),
        ("activity", "delete") => Some("delete"),
        ("athlete", "update")
            if event
                .updates
                .as_ref()
                .and_then(|updates| updates.get("authorized"))
                .is_some_and(|value| {
                    value.as_str() == Some("false") || value.as_bool() == Some(false)
                }) =>
        {
            Some("deauthorize")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_uses_its_offset_and_backfill_always_uses_fifteen_days() {
        let now = chrono::DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        assert_eq!(
            collection_after(None, false, now),
            (now - chrono::Duration::days(15)).timestamp()
        );
        assert_eq!(
            collection_after(Some(now), true, now),
            (now - chrono::Duration::days(15)).timestamp()
        );
        assert_eq!(
            collection_after(Some(now - chrono::Duration::days(2)), false, now),
            (now - chrono::Duration::days(2)).timestamp()
        );
    }

    fn event(
        object_type: &str,
        aspect_type: &str,
        updates: serde_json::Value,
    ) -> StravaWebhookEvent {
        serde_json::from_value(serde_json::json!({
            "object_type":object_type,"aspect_type":aspect_type,"updates":updates,
            "owner_id":35999641,"object_id":887654322,"subscription_id":1,"event_time":123
        }))
        .unwrap()
    }

    #[test]
    fn activity_callbacks_select_insert_update_and_delete_workflows() {
        for (aspect, expected) in [
            ("create", "insert"),
            ("update", "update"),
            ("delete", "delete"),
        ] {
            assert_eq!(
                webhook_action(&event("activity", aspect, serde_json::json!({}))),
                Some(expected)
            );
        }
    }

    #[test]
    fn privacy_update_uses_the_update_workflow() {
        assert_eq!(
            webhook_action(&event(
                "activity",
                "update",
                serde_json::json!({"private":"true"})
            )),
            Some("update")
        );
    }

    #[test]
    fn vendor_revocation_selects_cleanup_for_both_boolean_encodings() {
        for revoked in [serde_json::json!("false"), serde_json::json!(false)] {
            assert_eq!(
                webhook_action(&event(
                    "athlete",
                    "update",
                    serde_json::json!({"authorized":revoked})
                )),
                Some("deauthorize")
            );
        }
        assert_eq!(
            webhook_action(&event(
                "athlete",
                "update",
                serde_json::json!({"authorized":true})
            )),
            None
        );
    }
}
