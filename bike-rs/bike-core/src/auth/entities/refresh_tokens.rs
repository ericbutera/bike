use crate::auth::error::AuthError;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::Condition;
use sea_orm::DatabaseConnection;
use sea_orm::Set;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "refresh_tokens")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub token: String,
    pub user_pid: Uuid,
    pub expires_at: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn find_by_token(
        db: &DatabaseConnection,
        token: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(token).one(db).await
    }

    pub async fn remove_by_token(db: &DatabaseConnection, token: &str) -> Result<(), DbErr> {
        Entity::delete_by_id(token).exec(db).await?;
        Ok(())
    }

    pub async fn consume(db: &DatabaseConnection, token: &str) -> Result<bool, DbErr> {
        let deleted = Entity::delete_many()
            .filter(Column::Token.eq(token))
            .filter(Column::ExpiresAt.gt(chrono::Utc::now().timestamp()))
            .exec(db)
            .await?;
        Ok(deleted.rows_affected == 1)
    }

    pub async fn remove_by_user_pid(db: &DatabaseConnection, user_pid: &Uuid) -> Result<(), DbErr> {
        Entity::delete_many()
            .filter(Condition::all().add(Column::UserPid.eq(*user_pid)))
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn create_record(
        db: &DatabaseConnection,
        user_pid: Uuid,
        token: &str,
    ) -> Result<(), AuthError> {
        let am = ActiveModel {
            token: Set(token.to_owned()),
            user_pid: Set(user_pid),
            expires_at: Set((chrono::Utc::now() + chrono::Duration::days(7)).timestamp()),
            ..Default::default()
        };

        am.insert(db).await.map_err(|e| {
            AuthError::internal_error(format!("Failed to create refresh token: {}", e))
        })?;

        Ok(())
    }
}
