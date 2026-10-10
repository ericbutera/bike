use sea_orm::{
    entity::prelude::*,
    sea_query::{Expr, OnConflict},
    ConnectionTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, serde::Serialize, utoipa::ToSchema)]
#[schema(as = WorkerOutputEvidence)]
#[sea_orm(table_name = "processing_pipeline_outputs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub run_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub kind: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub target_id: i32,
    pub revision: String,
    pub required: bool,
    pub status: String,
    pub reason: Option<String>,
    #[schema(value_type = String, format = DateTime)]
    pub required_at: DateTimeUtc,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub available_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn advance(
        db: &impl ConnectionTrait,
        kind: &str,
        target_id: i32,
        revision: &str,
    ) -> Result<(), DbErr> {
        Entity::update_many()
            .col_expr(Column::Revision, Expr::value(revision))
            .filter(Column::Kind.eq(kind))
            .filter(Column::TargetId.eq(target_id))
            .filter(Column::Status.eq("pending"))
            .exec(db)
            .await?;
        Ok(())
    }
    /// Requirements and source changes share the caller's transaction. Updating a
    /// requirement invalidates prior readiness; publication must match its revision.
    pub async fn require(
        db: &impl ConnectionTrait,
        kind: &str,
        target_id: i32,
        revision: String,
    ) -> Result<(), DbErr> {
        let Some(context) = crate::background_jobs::pipeline::PipelineContext::current() else {
            return Ok(());
        };
        super::pipeline_runs::Model::record_origin(db, &context).await?;
        super::pipeline_subjects::Model::attach(db, &context.run_id, kind, target_id).await?;
        Entity::insert(ActiveModel {
            run_id: Set(context.run_id.clone()),
            kind: Set(kind.into()),
            target_id: Set(target_id),
            revision: Set(revision),
            required: Set(true),
            status: Set("pending".into()),
            reason: Set(None),
            required_at: Set(chrono::Utc::now()),
            available_at: Set(None),
        })
        .on_conflict(
            OnConflict::columns([Column::RunId, Column::Kind, Column::TargetId])
                .update_columns([
                    Column::Revision,
                    Column::Status,
                    Column::Reason,
                    Column::RequiredAt,
                    Column::AvailableAt,
                ])
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        super::pipeline_runs::Entity::update_many()
            .col_expr(
                super::pipeline_runs::Column::AvailableAt,
                Expr::value(None::<DateTimeUtc>),
            )
            .filter(super::pipeline_runs::Column::Id.eq(context.run_id))
            .exec(db)
            .await?;
        Ok(())
    }

    /// A committed publication can satisfy multiple contributing runs. Matching
    /// revisions are mandatory; discovery or task completion is never readiness.
    pub async fn publish(
        db: &impl ConnectionTrait,
        kind: &str,
        target_id: i32,
        revision: &str,
        published_at: DateTimeUtc,
    ) -> Result<(), DbErr> {
        Entity::update_many()
            .col_expr(Column::Status, Expr::value("available"))
            .col_expr(Column::AvailableAt, Expr::value(published_at))
            .filter(Column::Kind.eq(kind))
            .filter(Column::TargetId.eq(target_id))
            .filter(Column::Revision.eq(revision))
            .filter(Column::Status.eq("pending"))
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn attach_waiting_tasks(
        db: &impl ConnectionTrait,
        kind: &str,
        target_id: i32,
        revision: &str,
        task_id: i32,
    ) -> Result<(), DbErr> {
        let waiting = Entity::find()
            .filter(Column::Kind.eq(kind))
            .filter(Column::TargetId.eq(target_id))
            .filter(Column::Revision.eq(revision))
            .filter(Column::Status.eq("pending"))
            .order_by_asc(Column::RunId)
            .all(db)
            .await?;
        for output in waiting {
            super::pipeline_runs::Entity::find_by_id(&output.run_id)
                .lock_exclusive()
                .one(db)
                .await?;
            super::pipeline_tasks::Entity::insert(super::pipeline_tasks::ActiveModel {
                run_id: Set(output.run_id),
                task_id: Set(task_id),
                parent_task_id: Set(None),
            })
            .on_conflict(
                OnConflict::columns([
                    super::pipeline_tasks::Column::RunId,
                    super::pipeline_tasks::Column::TaskId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
        }
        Ok(())
    }
}
