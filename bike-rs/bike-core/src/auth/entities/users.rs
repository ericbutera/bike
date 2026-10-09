use crate::auth::error::AuthError;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{Condition, DatabaseConnection, DbErr};
use sea_orm::{IntoActiveModel, PaginatorTrait, QueryOrder, Set};
use uuid::Uuid;

pub const SYNTHETIC_PROVIDER: &str = "bike-synthetics";

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub pid: Uuid,
    pub email: String,
    pub password: Option<String>,
    pub api_key: String,
    pub name: String,
    pub is_admin: Option<bool>,
    pub disabled: bool,
    pub reset_token: Option<String>,
    pub reset_sent_at: Option<DateTime<Utc>>,
    pub email_verification_token: Option<String>,
    pub email_verification_sent_at: Option<DateTime<Utc>>,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub magic_link_token: Option<String>,
    pub magic_link_expiration: Option<DateTime<Utc>>,
    pub oauth_subject: Option<String>,
    pub oauth_provider: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn is_synthetic(&self) -> bool {
        self.oauth_provider.as_deref() == Some(SYNTHETIC_PROVIDER)
    }

    pub fn ensure_enabled(&self) -> Result<(), AuthError> {
        if self.disabled || self.is_synthetic() {
            return Err(AuthError::forbidden("this account is disabled"));
        }
        Ok(())
    }

    pub async fn admin_page(
        db: &DatabaseConnection,
        search: Option<&str>,
        disabled: Option<bool>,
        page: u64,
        per_page: u64,
    ) -> Result<(Vec<Self>, u64), DbErr> {
        let mut query = Entity::find().order_by_desc(Column::CreatedAt);
        if let Some(search) = search {
            let pattern = format!("%{search}%");
            query = query.filter(
                Condition::any()
                    .add(Column::Email.like(&pattern))
                    .add(Column::Name.like(&pattern)),
            );
        }
        if let Some(disabled) = disabled {
            query = query.filter(Column::Disabled.eq(disabled));
        }
        let paginator = query.paginate(db, per_page);
        let total = paginator.num_items().await?;
        Ok((paginator.fetch_page(page - 1).await?, total))
    }

    pub async fn find_by_id(db: &DatabaseConnection, id: i32) -> Result<Self, AuthError> {
        Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| AuthError::entity_not_found("User not found"))
    }

    pub async fn update_admin(
        self,
        db: &DatabaseConnection,
        name: Option<String>,
        is_admin: Option<bool>,
    ) -> Result<Self, DbErr> {
        let mut active = self.into_active_model();
        if let Some(name) = name {
            active.name = Set(name);
        }
        if let Some(is_admin) = is_admin {
            active.is_admin = Set(Some(is_admin));
        }
        active.update(db).await
    }

    pub async fn set_disabled(
        self,
        db: &DatabaseConnection,
        actor_id: i32,
        disabled: bool,
    ) -> Result<Self, AuthError> {
        if actor_id == self.id && disabled {
            return Err(AuthError::forbidden(
                "administrators cannot disable their own account",
            ));
        }
        let mut active = self.into_active_model();
        active.disabled = Set(disabled);
        Ok(active.update(db).await?)
    }

    pub async fn find_by_pid(db: &DatabaseConnection, pid: &Uuid) -> Result<Option<Self>, DbErr> {
        Entity::find().filter(Column::Pid.eq(*pid)).one(db).await
    }

    pub async fn find_by_email(
        db: &DatabaseConnection,
        email: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::Email.eq(email.to_string()))
            .one(db)
            .await
    }
}
