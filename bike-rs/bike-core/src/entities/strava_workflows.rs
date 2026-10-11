use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, QueryFilter, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "strava_workflows")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub connection_id: i32,
    pub athlete_id: i64,
    pub activity_id: Option<i64>,
    pub action: String,
    pub source: Json,
    pub checkpoint: Json,
    pub status: String,
    pub error: Option<String>,
    pub accepted_at: DateTimeUtc,
    pub completed_at: Option<DateTimeUtc>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn correlation_id(&self) -> String {
        self.activity_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| self.id.clone())
    }

    pub fn worker_context(&self) -> Json {
        serde_json::json!({
            "correlation_id": self.correlation_id(),
            "workflow_id": self.id,
            "parent_workflow_id": self.source["parent"],
            "strava_activity_id": self.activity_id,
            "athlete_id": self.athlete_id,
        })
    }

    pub async fn waiting(
        db: &impl ConnectionTrait,
        connection_id: i32,
    ) -> Result<Option<Json>, DbErr> {
        let rows = Entity::find()
            .filter(Column::ConnectionId.eq(connection_id))
            .filter(Column::Status.is_in(["accepted", "running"]))
            .all(db)
            .await?;
        for row in rows {
            if let Some(pause) = row.checkpoint.get("pause") {
                let at: DateTimeUtc = serde_json::from_value(pause["retry_at"].clone())
                    .map_err(|error| DbErr::Custom(error.to_string()))?;
                if at > chrono::Utc::now() {
                    return Ok(Some(pause.clone()));
                }
            }
        }
        Ok(None)
    }

    pub async fn pending_revocation(
        db: &impl ConnectionTrait,
        connection: &super::strava_connections::Model,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::ConnectionId.eq(connection.id))
            .filter(Column::Action.eq("deauthorize"))
            .filter(Column::AcceptedAt.gte(connection.created_at))
            .filter(Column::Status.ne("completed"))
            .one(db)
            .await
    }
    pub async fn active_collection(
        db: &impl ConnectionTrait,
        connection_id: i32,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::ConnectionId.eq(connection_id))
            .filter(Column::Action.is_in(["sync", "backfill"]))
            .filter(Column::Status.is_in(["accepted", "running"]))
            .one(db)
            .await
    }
    pub async fn checkpoint(&self, db: &impl ConnectionTrait, value: Json) -> Result<Self, DbErr> {
        let mut active: ActiveModel = self.clone().into();
        active.checkpoint = Set(value);
        active.status = Set("running".into());
        active.error = Set(None);
        active.update(db).await
    }
    pub async fn finish(
        &self,
        db: &impl ConnectionTrait,
        error: Option<String>,
    ) -> Result<Self, DbErr> {
        let mut active: ActiveModel = self.clone().into();
        active.status = Set(if error.is_some() {
            "failed"
        } else {
            "completed"
        }
        .into());
        active.completed_at = Set(if error.is_none() {
            Some(chrono::Utc::now())
        } else {
            None
        });
        active.error = Set(error);
        active.update(db).await
    }
}
