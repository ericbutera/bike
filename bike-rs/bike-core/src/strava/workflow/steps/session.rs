use super::super::{intents, Action, Run, Source};
use crate::{
    background_jobs::{background_tasks, pipeline::PipelineContext},
    config::Config,
    entities::{strava_activity_states, strava_connections, strava_workflows},
    strava::{client::StravaApiClient, objects::ObjectStore},
    workflow_error::WorkflowError,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction};

pub(super) struct Session<'a> {
    pub run: Run<'a>,
    pub pipeline: PipelineContext,
    lease: Option<DatabaseTransaction>,
}

impl<'a> Session<'a> {
    pub async fn load(
        db: &'a DatabaseConnection,
        id: &str,
        expected: Option<Action>,
    ) -> Result<Option<Self>, WorkflowError> {
        let row = load_row(db, id).await?;
        let action = row.action.parse::<Action>()?;
        if !matches!(action, Action::Upsert | Action::Delete)
            || expected.is_some_and(|expected| expected != action)
        {
            return Err(WorkflowError::bad_request(
                "Strava step does not match its operation",
            ));
        }
        let lease = strava_connections::Model::execution_lease(db, row.connection_id).await?;
        let row = load_row(db, id).await?;
        let source = Source::from_value(row.source.clone())?;
        let connection = strava_connections::Model::load(db, row.connection_id).await?;
        let Some(connection) =
            connection.filter(|connection| connection.created_at == source.generation)
        else {
            row.finish(db, None).await?;
            return Ok(None);
        };
        if row.status == "completed" {
            return Ok(None);
        }
        if expected.is_some() && superseded(db, &row, &source, &connection).await? {
            row.finish(db, None).await?;
            return Ok(None);
        }
        let store = ObjectStore::new(&Config::get().uploads_dir);
        source.verify(&store).await?;
        let pipeline = origin(db, &row).await?;
        let client = StravaApiClient::new(db, Config::get())?;
        Ok(Some(Self {
            run: Run {
                db,
                row,
                action,
                source,
                connection,
                client,
                store,
            },
            pipeline,
            lease,
        }))
    }

    pub async fn with_causal_context<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, WorkflowError>>,
    ) -> Result<T, WorkflowError> {
        PipelineContext::scope(
            Some(self.pipeline.clone()),
            intents::scope(&self.run.row.id, future),
        )
        .await
    }

    pub async fn reference(
        &self,
        stage: &str,
        input: &str,
    ) -> Result<crate::strava::objects::ObjectRef, WorkflowError> {
        let row = load_row(self.run.db, &self.run.row.id).await?;
        serde_json::from_value(row.checkpoint[format!("{stage}:v1:{input}")].clone())
            .map_err(super::super::stage::json_error)
    }

    pub fn activity_id(&self) -> Result<i64, WorkflowError> {
        self.run
            .row
            .activity_id
            .ok_or_else(|| WorkflowError::bad_request("Missing Strava activity ID"))
    }

    pub async fn release(self) -> Result<(), WorkflowError> {
        if let Some(lease) = self.lease {
            lease.commit().await?;
        }
        Ok(())
    }
}

pub(super) async fn load_row(
    db: &DatabaseConnection,
    id: &str,
) -> Result<strava_workflows::Model, WorkflowError> {
    strava_workflows::Model::load(db, id)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Strava operation was not found"))
}

async fn superseded(
    db: &DatabaseConnection,
    row: &strava_workflows::Model,
    source: &Source,
    connection: &strava_connections::Model,
) -> Result<bool, WorkflowError> {
    if strava_workflows::Model::pending_revocation(db, connection)
        .await?
        .is_some_and(|pending| source.parent.as_deref() != Some(pending.id.as_str()))
    {
        return Ok(true);
    }
    let Some(id) = row.activity_id else {
        return Ok(false);
    };
    Ok(strava_activity_states::Model::load(db, row.athlete_id, id)
        .await?
        .is_some_and(|state| {
            state.connection_generation == source.generation && state.event_time > source.event_time
        }))
}

pub(super) async fn origin(
    db: &DatabaseConnection,
    row: &strava_workflows::Model,
) -> Result<PipelineContext, WorkflowError> {
    if let Some(task) = background_tasks::Model::latest_strava_workflow_task(db, &row.id).await? {
        if let Some(origin) =
            PipelineContext::from_payload(&task.payload).map_err(super::super::stage::json_error)?
        {
            return Ok(origin);
        }
    }
    Ok(PipelineContext {
        run_id: row.id.clone(),
        pipeline_started_at: row.accepted_at,
        entrypoint: "strava_temporal".into(),
        request_id: None,
        parent_task_id: None,
        trace_context: None,
        gateway_history: None,
    })
}
