use crate::{app_error::AppError, storage::AppStorage};
use axum::{async_trait, extract::FromRequestParts, http::request::Parts, response::IntoResponse};
use bike_core::{auth::entities::users, auth::UserContext, entities::strava_connections};
use std::sync::Arc;

pub struct StravaConnection {
    pub user: users::Model,
    pub connection: strava_connections::Model,
}

pub struct OptionalStravaConnection {
    pub user: users::Model,
    pub connection: Option<strava_connections::Model>,
}

#[async_trait]
impl FromRequestParts<Arc<AppStorage>> for OptionalStravaConnection {
    type Rejection = axum::response::Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppStorage>) -> Result<Self, Self::Rejection> {
        let UserContext { user, .. } = UserContext::<AppStorage>::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        let connection = strava_connections::Model::for_user(&state.db, user.id)
            .await
            .map_err(AppError::from)
            .map_err(IntoResponse::into_response)?;
        Ok(Self { user, connection })
    }
}

#[async_trait]
impl FromRequestParts<Arc<AppStorage>> for StravaConnection {
    type Rejection = axum::response::Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppStorage>) -> Result<Self, Self::Rejection> {
        let OptionalStravaConnection { user, connection } =
            OptionalStravaConnection::from_request_parts(parts, state).await?;
        let connection = connection.ok_or_else(|| AppError::not_found("Connect Strava first").into_response())?;
        Ok(Self { user, connection })
    }
}
