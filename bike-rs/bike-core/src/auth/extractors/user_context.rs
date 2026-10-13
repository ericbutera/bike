use crate::auth::entities::users;
use crate::auth::error::AuthError;
use crate::auth::extractors::{ApiClientIdentity, AuthIdentity, AuthInfo, AuthStorage};
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::marker::PhantomData;
use std::sync::Arc;

/// Extractor for API clients - validates API client token
pub struct ApiClientContext<T = ()> {
    pub client: ApiClientIdentity,
    _phantom: PhantomData<T>,
}

impl<T> ApiClientContext<T> {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.client.has_scope(scope)
    }

    pub fn require_scope(&self, scope: &str) -> Result<(), AuthError> {
        if self.has_scope(scope) {
            Ok(())
        } else {
            Err(AuthError::forbidden(format!("Missing scope: {}", scope)))
        }
    }

    pub fn require_scopes(&self, scopes: &[&str]) -> Result<(), AuthError> {
        for scope in scopes {
            self.require_scope(scope)?;
        }
        Ok(())
    }
}

#[async_trait]
impl<S, T> FromRequestParts<S> for ApiClientContext<T>
where
    Arc<T>: FromRef<S>,
    T: AuthStorage,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth = AuthInfo::<T>::from_request_parts(parts, state).await?;
        match auth.identity {
            Some(AuthIdentity::ApiClient(client)) => Ok(ApiClientContext {
                client,
                _phantom: PhantomData,
            }),
            Some(AuthIdentity::User(_)) => {
                Err(AuthError::unauthorized("API client token required"))
            }
            None => Err(AuthError::unauthorized("Missing authentication")),
        }
    }
}

/// Extractor for authenticated users - validates token and loads user from database
pub struct UserContext<T = ()> {
    pub user: users::Model,
    _phantom: PhantomData<T>,
}

#[async_trait]
impl<S, T> FromRequestParts<S> for UserContext<T>
where
    Arc<T>: FromRef<S>,
    T: AuthStorage,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let storage: Arc<T> = FromRef::from_ref(state);
        let auth = AuthInfo::<T>::from_request_parts(parts, state).await?;

        let user_pid = match auth.identity {
            Some(AuthIdentity::User(identity)) => identity.user_pid,
            Some(AuthIdentity::ApiClient(_)) => {
                return Err(AuthError::unauthorized("User token required"))
            }
            None => return Err(AuthError::unauthorized("Missing authentication")),
        };

        let user = users::Entity::find()
            .filter(users::Column::Pid.eq(user_pid))
            .one(storage.db())
            .await
            .map_err(|e| AuthError::internal_error(format!("Failed to query user: {}", e)))?
            .ok_or_else(|| AuthError::unauthorized("User not found"))?;

        user.ensure_enabled()?;
        Ok(Self {
            user,
            _phantom: PhantomData,
        })
    }
}

/// Extractor for verified users - requires email verification
pub struct VerifiedUserContext<T = ()> {
    pub user: users::Model,
    _phantom: PhantomData<T>,
}

#[async_trait]
impl<S, T> FromRequestParts<S> for VerifiedUserContext<T>
where
    Arc<T>: FromRef<S>,
    T: AuthStorage,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let storage: Arc<T> = FromRef::from_ref(state);
        let auth = AuthInfo::<T>::from_request_parts(parts, state).await?;

        let user_pid = match auth.identity {
            Some(AuthIdentity::User(identity)) => identity.user_pid,
            Some(AuthIdentity::ApiClient(_)) => {
                return Err(AuthError::unauthorized("User token required"))
            }
            None => return Err(AuthError::unauthorized("Missing authentication")),
        };

        let user = users::Entity::find()
            .filter(users::Column::Pid.eq(user_pid))
            .one(storage.db())
            .await
            .map_err(|e| AuthError::internal_error(format!("Failed to query user: {}", e)))?
            .ok_or_else(|| AuthError::unauthorized("User not found"))?;

        user.ensure_enabled()?;
        if user.email_verified_at.is_none() {
            return Err(AuthError::forbidden("Email verification required"));
        }

        Ok(Self {
            user,
            _phantom: PhantomData,
        })
    }
}

/// Extractor for admin users - requires is_admin flag
pub struct AdminUserContext<T = ()> {
    pub user: users::Model,
    _phantom: PhantomData<T>,
}

#[async_trait]
impl<S, T> FromRequestParts<S> for AdminUserContext<T>
where
    Arc<T>: FromRef<S>,
    T: AuthStorage,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let storage: Arc<T> = FromRef::from_ref(state);
        let auth = AuthInfo::<T>::from_request_parts(parts, state).await?;

        let user_pid = match auth.identity {
            Some(AuthIdentity::User(identity)) => identity.user_pid,
            Some(AuthIdentity::ApiClient(_)) => {
                return Err(AuthError::unauthorized("User token required"))
            }
            None => return Err(AuthError::unauthorized("Missing authentication")),
        };

        let user = users::Entity::find()
            .filter(users::Column::Pid.eq(user_pid))
            .one(storage.db())
            .await
            .map_err(|e| AuthError::internal_error(format!("Failed to query user: {}", e)))?
            .ok_or_else(|| AuthError::unauthorized("User not found"))?;

        user.ensure_enabled()?;
        if !user.is_admin.unwrap_or(false) {
            return Err(AuthError::forbidden("Admin access required"));
        }

        Ok(Self {
            user,
            _phantom: PhantomData,
        })
    }
}

// background_jobs is already a dependency of `auth`, so we can implement
// AdminVerified here — satisfying the orphan rule (local type, external trait).
impl<S> crate::background_jobs::admin::AdminVerified for AdminUserContext<S> {}
