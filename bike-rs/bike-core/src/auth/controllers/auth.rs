use crate::auth::cookies::{clear_refresh_cookie_value, refresh_cookie_value, REFRESH_COOKIE_NAME};
use crate::auth::entities::refresh_tokens;
use crate::auth::error::AuthError;
use crate::auth::extractors::{AuthInfo, UserContext};
use crate::auth::services::{SessionService, UserResponse};
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use sea_orm::{DatabaseConnection, EntityTrait};
use std::sync::Arc;

pub trait AuthRouteStorage: Send + Sync + 'static {
    fn db(&self) -> &DatabaseConnection;
    fn session_service(&self) -> &SessionService;
    fn frontend_url(&self) -> &str;
}

pub fn session_routes<S>() -> Router<Arc<S>>
where
    S: AuthRouteStorage + crate::auth::extractors::AuthStorage,
{
    Router::new()
        .route("/auth/current", get(current::<S>))
        .route("/auth/refresh", post(refresh::<S>))
        .route("/auth/logout", get(logout::<S>))
}
fn extract_cookie(headers: &HeaderMap, name: &str) -> Result<String, AuthError> {
    let cookies = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    for cookie in cookies.split(';') {
        let cookie = cookie.trim();
        if let Some(value) = cookie.strip_prefix(&format!("{}=", name)) {
            return Ok(value.to_string());
        }
    }

    Err(AuthError::unauthorized(format!("Missing cookie: {}", name)))
}

fn trim_ascii_whitespace(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map(|index| index + 1)
        .unwrap_or(start);

    &bytes[start..end]
}

fn refresh_body_uses_cookie(body: &[u8]) -> bool {
    let trimmed = trim_ascii_whitespace(body);

    trimmed.is_empty() || trimmed == b"\"\"" || trimmed == b"null" || trimmed == b"{}"
}

#[utoipa::path(
    get,
    path = "/auth/current",
    responses(
        (status = 200, description = "Current user info", body = UserResponse),
        (status = 401, description = "Not authenticated")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "auth"
)]
pub async fn current<S>(
    UserContext { user, .. }: UserContext<S>,
) -> Result<Json<UserResponse>, AuthError>
where
    S: crate::auth::extractors::AuthStorage,
{
    Ok(Json(user.into()))
}

#[utoipa::path(
    post,
    path = "/auth/refresh",
    params(),
    request_body(content = String, description = "Optional refresh token in body or cookie", content_type = "application/json"),
    responses(
        (status = 200, description = "Token refreshed successfully"),
        (status = 401, description = "Invalid refresh token")
    ),
    tag = "auth"
)]
pub async fn refresh<S>(
    State(state): State<Arc<S>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AuthError>
where
    S: AuthRouteStorage,
{
    let used_cookie = refresh_body_uses_cookie(&body);
    let refresh_token = if used_cookie {
        extract_cookie(&headers, REFRESH_COOKIE_NAME)?
    } else {
        #[derive(serde::Deserialize)]
        struct RefreshRequest {
            refresh_token: String,
        }
        let rr: RefreshRequest = serde_json::from_slice(&body)
            .map_err(|_| AuthError::validation("Invalid refresh token payload"))?;
        rr.refresh_token
    };

    let token = state
        .session_service()
        .refresh(state.db(), refresh_token)
        .await?;

    let mut builder = Response::builder().status(StatusCode::OK);
    if used_cookie {
        let cookie_val = refresh_cookie_value(&token.refresh_token, state.frontend_url());
        builder = builder.header(axum::http::header::SET_COOKIE, cookie_val);

        let user_obj = serde_json::json!({
            "pid": token.pid,
            "name": token.name,
            "email": token.email,
            "is_admin": token.is_admin,
        });
        let body = serde_json::json!({ "user": user_obj });
        let body_bytes = serde_json::to_vec(&body).map_err(|e| {
            AuthError::internal_error(format!("Failed to serialize response: {}", e))
        })?;

        let response = builder
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(body_bytes))
            .map_err(|e| AuthError::internal_error(format!("Failed to build response: {}", e)))?;

        return Ok(response);
    }

    let body = serde_json::json!({ "message": "ok" });
    let body_bytes = serde_json::to_vec(&body)
        .map_err(|e| AuthError::internal_error(format!("Failed to serialize response: {}", e)))?;

    let response = builder
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body_bytes))
        .map_err(|e| AuthError::internal_error(format!("Failed to build response: {}", e)))?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::refresh_body_uses_cookie;

    #[test]
    fn cookie_refresh_accepts_empty_body_shapes() {
        assert!(refresh_body_uses_cookie(b""));
        assert!(refresh_body_uses_cookie(b"   \n\t"));
        assert!(refresh_body_uses_cookie(br#""""#));
        assert!(refresh_body_uses_cookie(b"null"));
        assert!(refresh_body_uses_cookie(b"{}"));
    }

    #[test]
    fn explicit_refresh_token_body_does_not_use_cookie() {
        assert!(!refresh_body_uses_cookie(
            br#"{"refresh_token":"refresh-token"}"#
        ));
    }
}

#[utoipa::path(
    get,
    path = "/auth/logout",
    responses(
        (status = 200, description = "Logout successful"),
        (status = 401, description = "Not authenticated")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "auth"
)]
pub async fn logout<S>(
    State(state): State<Arc<S>>,
    auth: AuthInfo<S>,
) -> Result<Response, AuthError>
where
    S: AuthRouteStorage + crate::auth::extractors::AuthStorage,
{
    if let Some(refresh_token) = auth.refresh_token {
        let _ = refresh_tokens::Entity::delete_by_id(refresh_token)
            .exec(crate::auth::extractors::AuthStorage::db(&*state))
            .await;
    } else if let Some(crate::auth::extractors::AuthIdentity::User(user_identity)) = auth.identity {
        let _ = state.session_service().logout(user_identity.user_pid).await;
    }

    let cookie_val = clear_refresh_cookie_value(state.frontend_url());

    let body = serde_json::json!({ "message": "ok" });
    let body_bytes = serde_json::to_vec(&body)
        .map_err(|e| AuthError::internal_error(format!("Failed to serialize response: {}", e)))?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::SET_COOKIE, cookie_val)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body_bytes))
        .map_err(|e| AuthError::internal_error(format!("Failed to build response: {}", e)))?;

    Ok(response)
}
