use super::{intents, Action, Execution, Run, Source};
use crate::strava::{client::StravaApiClient, objects::ObjectStore};
use crate::{
    config::Config,
    entities::{strava_connections, strava_workflows},
    workflow_error::WorkflowError,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use tracing::Instrument;

/// Compatibility entry point for previously retained generic workflow jobs.
pub async fn execute(db: &DatabaseConnection, id: &str) -> Result<Execution, WorkflowError> {
    let row = load(db, id).await?;
    execute_for(db, id, row.action.parse()?).await
}

/// Each registered worker processor accepts exactly its own operation.
pub async fn execute_for(
    db: &DatabaseConnection,
    id: &str,
    action: Action,
) -> Result<Execution, WorkflowError> {
    let row = load(db, id).await?;
    if row.action.parse::<Action>()? != action {
        return Err(WorkflowError::bad_request(
            "Strava processor does not match its workflow operation",
        ));
    }
    if row.status == "completed" {
        return Ok(Execution::Completed);
    }
    let lease = strava_connections::Model::execution_lease(db, row.connection_id).await?;
    let row = load(db, id).await?;
    if row.status == "completed" {
        if let Some(lease) = lease {
            lease.commit().await?;
        }
        return Ok(Execution::Completed);
    }
    let row = clear_pause(db, row).await?;
    let source = Source::from_value(row.source.clone())?;
    let connection = strava_connections::Entity::find_by_id(row.connection_id)
        .one(db)
        .await?;
    let Some(connection) =
        connection.filter(|connection| connection.created_at == source.generation)
    else {
        row.finish(db, None).await?;
        if let Some(lease) = lease {
            lease.commit().await?;
        }
        return Ok(Execution::Completed);
    };
    if superseded_by_disconnect(db, &connection, action, &source).await? {
        row.finish(db, None).await?;
        if let Some(lease) = lease {
            lease.commit().await?;
        }
        return Ok(Execution::Completed);
    }
    let result = execute_owned(db, row.clone(), source, connection, action).await;
    match &result {
        Ok(Execution::Completed) => {
            row.finish(db, None).await?;
        }
        Ok(Execution::WaitingForChildren) => {}
        Err(error) => {
            row.finish(db, Some(error.message.clone())).await?;
        }
    }
    if let Some(lease) = lease {
        lease.commit().await?;
    }
    result
}

async fn load(db: &DatabaseConnection, id: &str) -> Result<strava_workflows::Model, WorkflowError> {
    strava_workflows::Model::load(db, id)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Strava workflow was not found"))
}

async fn clear_pause(
    db: &DatabaseConnection,
    row: strava_workflows::Model,
) -> Result<strava_workflows::Model, WorkflowError> {
    if row.checkpoint.get("pause").is_none() {
        return Ok(row);
    }
    let mut checkpoint = row.checkpoint.clone();
    checkpoint
        .as_object_mut()
        .ok_or_else(|| WorkflowError::internal("Invalid Strava workflow checkpoint"))?
        .remove("pause");
    Ok(row.checkpoint(db, checkpoint).await?)
}

async fn superseded_by_disconnect(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
    action: Action,
    source: &Source,
) -> Result<bool, WorkflowError> {
    if action == Action::Disconnect {
        return Ok(false);
    }
    let pending = strava_workflows::Model::pending_revocation(db, connection).await?;
    // Disconnect's deletion tasks must run before removing the connection.
    Ok(pending.is_some_and(|pending| source.parent.as_deref() != Some(pending.id.as_str())))
}

async fn execute_owned(
    db: &DatabaseConnection,
    row: strava_workflows::Model,
    source: Source,
    connection: strava_connections::Model,
    action: Action,
) -> Result<Execution, WorkflowError> {
    let store = ObjectStore::new(&Config::get().uploads_dir);
    source.verify(&store).await?;
    let client = StravaApiClient::new(db, Config::get())?;
    let run = Run {
        db,
        row,
        action,
        source,
        connection,
        client,
        store,
    };
    let span = tracing::info_span!("strava.workflow", correlation_id = %run.row.correlation_id(), workflow_id = %run.row.id, parent_workflow_id = run.source.parent.as_deref(), action = action.as_str(), athlete_id = run.row.athlete_id, strava_activity_id = run.row.activity_id, connection_id = run.row.connection_id, trace_id = tracing::field::Empty, span_id = tracing::field::Empty);
    crate::observability::record_span_trace_context(&span);
    intents::scope(
        &run.row.id,
        async {
            match action {
                Action::Sync | Action::Backfill => run.collection().await,
                Action::Disconnect => run.deauthorize().await,
                Action::Create | Action::Update | Action::Upsert | Action::Delete => {
                    run.activity().await?;
                    Ok(Execution::Completed)
                }
            }
        }
        .instrument(span),
    )
    .await
}
