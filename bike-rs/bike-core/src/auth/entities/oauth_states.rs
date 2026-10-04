use crate::auth::error::AuthError;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::Set;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "oauth_states")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub state_hash: String,
    pub provider: String,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn hash(state: &str) -> String {
        hex::encode(Sha256::digest(state.as_bytes()))
    }

    pub async fn save(
        db: &DatabaseConnection,
        provider: &str,
        state: &str,
        nonce: &str,
    ) -> Result<(), AuthError> {
        Entity::delete_many()
            .filter(Column::ExpiresAt.lte(Utc::now()))
            .exec(db)
            .await?;
        ActiveModel {
            state_hash: Set(Self::hash(state)),
            provider: Set(provider.to_string()),
            nonce: Set(nonce.to_string()),
            expires_at: Set(Utc::now() + chrono::Duration::minutes(10)),
            created_at: Set(Utc::now()),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(())
    }

    pub async fn consume(
        db: &DatabaseConnection,
        provider: &str,
        state: &str,
    ) -> Result<String, AuthError> {
        let invalid = || AuthError::bad_request("OAuth state is invalid or expired");
        if state.is_empty() {
            return Err(invalid());
        }
        let stored = Entity::find()
            .filter(Column::StateHash.eq(Self::hash(state)))
            .filter(Column::Provider.eq(provider))
            .filter(Column::ExpiresAt.gt(Utc::now()))
            .one(db)
            .await?
            .ok_or_else(invalid)?;
        let deleted = Entity::delete_many()
            .filter(Column::Id.eq(stored.id))
            .filter(Column::ExpiresAt.gt(Utc::now()))
            .exec(db)
            .await?;
        if deleted.rows_affected != 1 {
            return Err(invalid());
        }
        Ok(stored.nonce)
    }
}
