use crate::auth::cookies::REFRESH_COOKIE_NAME;
use crate::auth::entities::{api_clients, refresh_tokens};
use crate::auth::error::AuthError;
use crate::auth::tokens::{verify_access_token, TokenType};
use axum::http::request::Parts;
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
};
use sea_orm::{DatabaseConnection, EntityTrait};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ApiClientIdentity {
    pub client_id: Uuid,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UserIdentity {
    pub user_pid: Uuid,
}

#[derive(Debug, Clone)]
pub enum AuthIdentity {
    User(UserIdentity),
    Synthetic(UserIdentity),
    ApiClient(ApiClientIdentity),
}

pub struct AuthInfo<T: AuthStorage> {
    pub identity: Option<AuthIdentity>,
    /// If the request used a refresh cookie, this contains the token value
    pub refresh_token: Option<String>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: AuthStorage> AuthInfo<T> {
    fn new(identity: Option<AuthIdentity>, refresh_token: Option<String>) -> Self {
        Self {
            identity,
            refresh_token,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl ApiClientIdentity {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

/// Storage trait for authentication extractors
///
/// Provides database access and JWT secret for token verification.
/// Implement this trait to integrate with your application's storage layer.
pub trait AuthStorage: Send + Sync {
    fn db(&self) -> &DatabaseConnection;
    fn jwt_secret(&self) -> &str;

    /// Return the local development user identity when the application has
    /// explicitly enabled local-admin mode. Production implementations should
    /// leave this disabled.
    fn local_admin_user_pid(&self) -> Option<Uuid> {
        None
    }

    fn synthetic_auth(&self) -> Option<&crate::synthetics::SyntheticAuth> {
        None
    }
}

fn extract_cookie(headers: &axum::http::HeaderMap, name: &str) -> Result<String, AuthError> {
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

#[async_trait]
impl<S, T> FromRequestParts<S> for AuthInfo<T>
where
    Arc<T>: FromRef<S>,
    T: AuthStorage,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let storage: Arc<T> = FromRef::from_ref(state);
        let headers = &parts.headers;
        let mut cookie_error: Option<AuthError> = None;

        if let Some(value) = headers.get(crate::synthetics::AUTH_HEADER) {
            let config = storage
                .synthetic_auth()
                .ok_or_else(|| AuthError::unauthorized("Synthetic authentication is disabled"))?;
            verify_synthetic_request(parts, value, &config.key)?;
            return Ok(AuthInfo::new(
                Some(AuthIdentity::Synthetic(UserIdentity {
                    user_pid: config.user_pid,
                })),
                None,
            ));
        }

        if let Some(user_pid) = storage.local_admin_user_pid() {
            return Ok(AuthInfo::new(
                Some(AuthIdentity::User(UserIdentity { user_pid })),
                None,
            ));
        }

        if let Ok(token) = extract_cookie(headers, REFRESH_COOKIE_NAME) {
            match verify_refresh_token(storage.db(), &token).await {
                Ok(user_pid) => {
                    return Ok(AuthInfo::new(
                        Some(AuthIdentity::User(UserIdentity { user_pid })),
                        Some(token),
                    ))
                }
                Err(err) => {
                    cookie_error = Some(err);
                }
            }
        }

        if let Some(auth_hdr) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
            if let Some(tok) = auth_hdr.strip_prefix("Bearer ") {
                let secret = storage.jwt_secret();
                match verify_access_token(tok, secret) {
                    Ok(td) => match td.claims.token_type {
                        TokenType::User => {
                            let user_pid = Uuid::parse_str(&td.claims.sub)
                                .map_err(|_| AuthError::unauthorized("Invalid token"))?;
                            return Ok(AuthInfo::new(
                                Some(AuthIdentity::User(UserIdentity { user_pid })),
                                None,
                            ));
                        }
                        TokenType::ApiClient => {
                            let client_id = Uuid::parse_str(&td.claims.sub)
                                .map_err(|_| AuthError::unauthorized("Invalid token"))?;

                            // Verify client exists and get scopes
                            let (scopes, revoked) =
                                verify_api_client(storage.db(), &client_id).await?;

                            if revoked {
                                return Err(AuthError::unauthorized("API client revoked"));
                            }

                            return Ok(AuthInfo::new(
                                Some(AuthIdentity::ApiClient(ApiClientIdentity {
                                    client_id,
                                    scopes,
                                })),
                                None,
                            ));
                        }
                    },
                    Err(_) => return Err(AuthError::unauthorized("Invalid token")),
                }
            }
        }

        if let Some(err) = cookie_error {
            return Err(err);
        }

        Ok(AuthInfo::new(None, None))
    }
}

fn verify_synthetic_request(
    parts: &Parts,
    supplied: &axum::http::HeaderValue,
    expected: &str,
) -> Result<(), AuthError> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    // Public ingress strips the synthetic header. Also reject forwarded traffic
    // here so a public request cannot become an internal request through a proxy.
    if parts.headers.contains_key("x-forwarded-for") || parts.headers.contains_key("forwarded") {
        return Err(AuthError::forbidden(
            "Synthetic authentication is internal only",
        ));
    }
    let mut expected_mac = Hmac::<Sha256>::new_from_slice(expected.as_bytes()).expect("HMAC key");
    let mut supplied_mac = Hmac::<Sha256>::new_from_slice(supplied.as_bytes()).expect("HMAC key");
    expected_mac.update(b"bike synthetic request");
    supplied_mac.update(b"bike synthetic request");
    expected_mac
        .verify_slice(&supplied_mac.finalize().into_bytes())
        .map_err(|_| AuthError::unauthorized("Invalid synthetic credential"))?;
    let path = parts
        .extensions
        .get::<axum::extract::OriginalUri>()
        .map_or(parts.uri.path(), |original| original.0.path());
    if !synthetic_read_allowed(&parts.method, path) {
        return Err(AuthError::forbidden(
            "Synthetic requests are limited to the browser read journey",
        ));
    }
    Ok(())
}

fn synthetic_read_allowed(method: &axum::http::Method, path: &str) -> bool {
    if *method != axum::http::Method::GET {
        return false;
    }
    if matches!(
        path,
        "/api/auth/current"
            | "/api/preferences"
            | "/api/activities"
            | "/api/segments"
            | "/api/synthetics/scenario"
            | "/api/activity-imports"
            | "/api/activity-imports/processing-state"
    ) {
        return true;
    }
    let pieces = path.trim_start_matches('/').split('/').collect::<Vec<_>>();
    matches!(pieces.as_slice(), ["api", "activities" | "segments", id] if id.parse::<i32>().is_ok_and(|id| id > 0))
        || matches!(pieces.as_slice(), ["api", "segments", id, "comparison"] if id.parse::<i32>().is_ok_and(|id| id > 0))
}

async fn verify_refresh_token(db: &DatabaseConnection, token: &str) -> Result<Uuid, AuthError> {
    let rt = refresh_tokens::Entity::find_by_id(token.to_string())
        .one(db)
        .await?
        .ok_or_else(|| AuthError::unauthorized("Invalid refresh token"))?;

    Ok(rt.user_pid)
}

async fn verify_api_client(
    db: &DatabaseConnection,
    client_id: &Uuid,
) -> Result<(Vec<String>, bool), AuthError> {
    let client = api_clients::Model::find_by_client_id(db, client_id)
        .await?
        .ok_or_else(|| AuthError::entity_not_found("api client not found"))?;

    Ok((client.parsed_scopes(), client.revoked_at.is_some()))
}
