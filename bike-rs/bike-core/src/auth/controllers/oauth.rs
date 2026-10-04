use crate::auth::cookies::{
    clear_oauth_state_cookie_value, oauth_state_cookie_value, refresh_cookie_value,
    validate_oauth_state,
};
use crate::auth::error::AuthError;
use crate::auth::services::oauth::OAuthUserInfo;
use crate::auth::services::oauth_provider_service::OAuthProviderService;
use crate::auth::services::provider_settings::{normalize_provider_id, PROVIDER_DEV};
use crate::auth::AuthRouteStorage;
use crate::auth::OAuthService;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

/// Generic storage trait for OAuth routes
pub trait OAuthRouteStorage: AuthRouteStorage {
    fn api_url(&self) -> &str;
    fn oauth_enabled(&self) -> bool {
        true
    }

    fn oauth_provider_enabled(&self, provider: &str) -> bool {
        self.oauth_enabled() && OAuthProviderService::is_provider_enabled(provider)
    }
}

pub fn routes<S>() -> Router<Arc<S>>
where
    S: OAuthRouteStorage,
{
    Router::new()
        .route("/providers", get(oauth_providers::<S>))
        .route("/:provider", get(oauth_authorize::<S>))
        .route("/:provider/callback", get(oauth_callback::<S>))
}

#[derive(Serialize, ToSchema)]
pub struct OAuthProvidersResponse {
    pub providers: Vec<crate::auth::services::oauth_provider_service::OAuthProviderMetadata>,
}

/// List OAuth providers enabled by environment configuration
#[utoipa::path(
    get,
    path = "/oauth/providers",
    responses(
        (status = 200, description = "Configured OAuth providers", body = OAuthProvidersResponse)
    ),
    tag = "oauth"
)]
pub async fn oauth_providers<S>(
    State(state): State<Arc<S>>,
) -> Result<Json<OAuthProvidersResponse>, AuthError>
where
    S: OAuthRouteStorage,
{
    let providers = if state.oauth_enabled() {
        OAuthProviderService::enabled_providers()
    } else {
        Vec::new()
    };

    Ok(Json(OAuthProvidersResponse { providers }))
}

/// Initiate provider OAuth flow - redirect user to provider login
#[utoipa::path(
    get,
    path = "/oauth/{provider}",
    params(
        ("provider" = String, Path, description = "OAuth provider name")
    ),
    responses(
        (status = 302, description = "Redirect to provider")
    ),
    tag = "oauth"
)]
pub async fn oauth_authorize<S>(
    State(state): State<Arc<S>>,
    Path(provider): Path<String>,
) -> Result<Response, AuthError>
where
    S: OAuthRouteStorage,
{
    let provider = normalize_provider_id(&provider)?;

    if !state.oauth_provider_enabled(&provider) {
        return Err(AuthError::forbidden("OAuth is disabled"));
    }

    if provider == PROVIDER_DEV {
        return complete_oauth_login(state, &provider, OAuthService::local_dev_user_info()).await;
    }

    let auth_url =
        OAuthService::get_authorization_url(state.db(), &provider, state.api_url()).await?;
    let cookie = oauth_state_cookie_value(&auth_url.state, state.frontend_url());
    Ok((
        [(header::SET_COOKIE, cookie)],
        Redirect::temporary(&auth_url.url),
    )
        .into_response())
}

#[derive(Deserialize, IntoParams)]
pub struct OAuthCallbackQuery {
    code: String,
    state: Option<String>,
}

/// Handle OAuth callback for generic provider
#[utoipa::path(
    get,
    path = "/oauth/{provider}/callback",
    params(
        ("provider" = String, Path, description = "OAuth provider name"),
        OAuthCallbackQuery
    ),
    responses(
        (status = 302, description = "Redirect to frontend")
    ),
    tag = "oauth"
)]
pub async fn oauth_callback<S>(
    State(state): State<Arc<S>>,
    Path(provider): Path<String>,
    Query(params): Query<OAuthCallbackQuery>,
    headers: HeaderMap,
) -> Result<Response, AuthError>
where
    S: OAuthRouteStorage,
{
    let provider = normalize_provider_id(&provider)?;

    if !state.oauth_provider_enabled(&provider) {
        return Err(AuthError::forbidden("OAuth is disabled"));
    }
    let callback_state = validate_oauth_state(params.state.as_deref(), &headers)?;
    let provider_user = OAuthService::exchange_code_and_get_user(
        state.db(),
        &provider,
        params.code,
        state.api_url(),
        &callback_state,
    )
    .await?;

    complete_oauth_login(state, &provider, provider_user).await
}

async fn complete_oauth_login<S>(
    state: Arc<S>,
    provider: &str,
    provider_user: OAuthUserInfo,
) -> Result<Response, AuthError>
where
    S: OAuthRouteStorage,
{
    let user =
        OAuthService::find_or_create_provider_user(state.db(), provider, provider_user).await?;

    let tokens = state
        .session_service()
        .issue_tokens(state.db(), &user)
        .await?;
    let frontend_url = state.frontend_url().to_string();
    let cookie_val = refresh_cookie_value(&tokens.refresh_token, &frontend_url);

    let redirect_url = format!("{}/auth/callback", frontend_url.trim_end_matches('/'));

    let response = Response::builder()
        .status(StatusCode::FOUND)
        .header(axum::http::header::LOCATION, redirect_url)
        .header(axum::http::header::SET_COOKIE, cookie_val)
        .header(
            axum::http::header::SET_COOKIE,
            clear_oauth_state_cookie_value(&frontend_url),
        )
        .body(Body::empty())
        .map_err(|e| {
            AuthError::internal_error(format!("Failed to build redirect response: {}", e))
        })?;

    Ok(response)
}
