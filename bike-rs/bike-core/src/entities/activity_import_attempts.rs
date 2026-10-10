use sea_orm::entity::prelude::*;
use sea_orm::{sea_query::Expr, ConnectionTrait, Iterable, QueryOrder, QuerySelect, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_import_attempts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub activity_import_id: i32,
    pub activity_id: Option<i32>,
    pub worker_task_id: Option<i32>,
    pub status: String,
    pub requested_stage: String,
    pub start_stage: String,
    pub current_stage: String,
    pub reused_attempt_id: Option<i32>,
    pub source_json: Json,
    pub stages_json: Json,
    pub checkpoint_json: Option<Json>,
    pub error: Option<String>,
    pub created_at: DateTimeUtc,
    pub started_at: Option<DateTimeUtc>,
    pub finished_at: Option<DateTimeUtc>,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub async fn for_worker_metadata(
        db: &impl ConnectionTrait,
        task_id: i32,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .select_only()
            .columns(
                Column::iter().filter(|column| {
                    !matches!(column, Column::CheckpointJson | Column::SourceJson)
                }),
            )
            .expr_as(Expr::value(None::<Json>), Column::CheckpointJson)
            .expr_as(Expr::value(serde_json::json!({})), Column::SourceJson)
            .filter(Column::WorkerTaskId.eq(task_id))
            .order_by_asc(Column::Id)
            .limit(100)
            .all(db)
            .await
    }
    pub async fn recent_metadata(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_id: i32,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .select_only()
            .columns(Column::iter().filter(|column| !matches!(column, Column::CheckpointJson)))
            .expr_as(Expr::cust("NULL"), Column::CheckpointJson)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityImportId.eq(import_id))
            .order_by_desc(Column::Id)
            .limit(50)
            .all(db)
            .await
    }

    pub async fn recent(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_id: i32,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityImportId.eq(import_id))
            .order_by_desc(Column::Id)
            .limit(50)
            .all(db)
            .await
    }

    pub async fn active(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_id: i32,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityImportId.eq(import_id))
            .filter(Column::Status.is_in(["queued", "running"]))
            .order_by_desc(Column::Id)
            .one(db)
            .await
    }

    pub async fn claim(db: &impl ConnectionTrait, id: i32) -> Result<bool, DbErr> {
        Ok(Self::update_many()
            .set(ActiveModel {
                status: Set("running".into()),
                started_at: Set(Some(chrono::Utc::now())),
                updated_at: Set(chrono::Utc::now()),
                ..Default::default()
            })
            .filter(Column::Id.eq(id))
            .filter(Column::Status.eq("queued"))
            .exec(db)
            .await?
            .rows_affected
            == 1)
    }
}
