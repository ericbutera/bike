use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "api_clients")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub client_id: Uuid,
    pub client_secret_hash: String,
    pub name: String,
    pub description: Option<String>,
    pub scopes: Json,
    pub owner_user_id: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::OwnerUserId",
        to = "super::users::Column::Id"
    )]
    Owner,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn find_by_client_id(
        db: &DatabaseConnection,
        client_id: &Uuid,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::ClientId.eq(*client_id))
            .one(db)
            .await
    }

    pub fn parsed_scopes(&self) -> Vec<String> {
        self.scopes
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }
}
