use crate::auth::entities::users;
use crate::auth::error::AuthError;
use crate::auth::services::provider_settings;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::AuthType;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::Instrument;

pub(crate) mod oidc;

#[derive(Debug, Serialize)]
pub struct OAuthAuthorizeUrl {
    pub url: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct OAuthUserInfo {
    #[serde(alias = "sub", alias = "id")]
    pub subject: String,
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub preferred_username: Option<String>,
    #[serde(default, alias = "email_verified")]
    pub verified_email: Option<bool>,
}

impl OAuthUserInfo {
    pub(crate) fn display_name(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.preferred_username.clone())
            .unwrap_or_else(|| self.email.clone())
    }
}

pub struct OAuthService;

impl OAuthService {
    fn create_client_from_config(cfg: provider_settings::ProviderConfig) -> Result<BasicClient, AuthError> {
        let auth_url = AuthUrl::new(cfg.auth_url.clone())
            .map_err(|e| AuthError::internal_error(format!("Invalid auth URL: {}", e)))?;

        let token_url = TokenUrl::new(cfg.token_url.clone())
            .map_err(|e| AuthError::internal_error(format!("Invalid token URL: {}", e)))?;

        let redirect_url = RedirectUrl::new(cfg.redirect_url.clone())
            .map_err(|e| AuthError::internal_error(format!("Invalid redirect URL: {}", e)))?;

        tracing::debug!(redirect_url = %cfg.redirect_url, "OAuth redirect URL configured");

        Ok(BasicClient::new(
            ClientId::new(cfg.client_id),
            Some(ClientSecret::new(cfg.client_secret)),
            auth_url,
            Some(token_url),
        )
        .set_redirect_uri(redirect_url)
        // Use request-body client authentication (client_secret_post) to
        // ensure compatibility with providers that expect form-encoded body.
        .set_auth_type(AuthType::RequestBody))
    }

    /// Build an authorization URL from an explicit ProviderConfig (without reading DB).
    pub async fn build_authorization_url_from_config(
        cfg: &provider_settings::ProviderConfig,
    ) -> Result<oauth2::url::Url, AuthError> {
        let client = Self::create_client_from_config(cfg.clone())?;

        let mut auth_req = client.authorize_url(CsrfToken::new_random);

        for scope in &cfg.scopes {
            auth_req = auth_req.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, _csrf_token) = auth_req.url();
        Ok(auth_url.clone())
    }

    /// Generate authorization URL for a named provider.
    pub async fn get_authorization_url(
        db: &DatabaseConnection,
        provider: &str,
        api_url: &str,
    ) -> Result<OAuthAuthorizeUrl, AuthError> {
        let cfg = provider_settings::get_provider_config(provider, api_url).await?;

        let client = Self::create_client_from_config(cfg.clone())?;

        let nonce = CsrfToken::new_random();
        let mut auth_req = client
            .authorize_url(CsrfToken::new_random)
            .add_extra_param("nonce", nonce.secret());

        for scope in cfg.scopes {
            auth_req = auth_req.add_scope(Scope::new(scope));
        }

        let (auth_url, csrf_token) = auth_req.url();

        crate::auth::entities::oauth_states::Model::save(db, provider, csrf_token.secret(), nonce.secret()).await?;

        Ok(OAuthAuthorizeUrl {
            url: auth_url.to_string(),
            state: csrf_token.secret().clone(),
        })
    }

    /// Exchange authorization code for access token and get user info for a provider.
    pub async fn exchange_code_and_get_user(
        db: &DatabaseConnection,
        provider: &str,
        code: String,
        api_url: &str,
        state: &str,
    ) -> Result<OAuthUserInfo, AuthError> {
        async move {
            let nonce = crate::auth::entities::oauth_states::Model::consume(db, provider, state).await?;
            let cfg = provider_settings::get_provider_config(provider, api_url).await?;
            if cfg.issuer_url.is_some() {
                return oidc::exchange(&cfg, code, nonce).await;
            }
            let client = Self::create_client_from_config(cfg.clone())?;

            let token_span = tracing::info_span!("oauth.token_exchange", oauth.provider = provider);
            let token_result = client
                .exchange_code(AuthorizationCode::new(code))
                .request_async(async_http_client)
                .instrument(token_span)
                .await
                .map_err(|e| AuthError::internal_error(format!("Token exchange failed: {}", e)))?;

            let userinfo_span = tracing::info_span!("oauth.userinfo.fetch", oauth.provider = provider);
            let user_info: OAuthUserInfo = async {
                reqwest::Client::new()
                    .get(&cfg.userinfo_url)
                    .bearer_auth(token_result.access_token().secret())
                    .send()
                    .await
                    .map_err(|e| AuthError::internal_error(format!("Failed to fetch user info: {}", e)))?
                    .json()
                    .await
                    .map_err(|e| AuthError::internal_error(format!("Failed to parse user info: {}", e)))
            }
            .instrument(userinfo_span)
            .await?;

            if user_info.verified_email == Some(false) {
                return Err(AuthError::validation("Email not verified by provider"));
            }

            Ok(user_info)
        }
        .instrument(tracing::info_span!("oauth.exchange_code", oauth.provider = provider))
        .await
    }

    /// Find or create user from an OAuth/OIDC provider.
    pub async fn find_or_create_provider_user(
        db: &DatabaseConnection,
        provider: &str,
        provider_user: OAuthUserInfo,
    ) -> Result<users::Model, AuthError> {
        let span = tracing::info_span!(
            "oauth.user.resolve",
            oauth.provider = provider,
            auth.user_resolution = tracing::field::Empty,
        );

        async {
            // First, try to find user by provider subject.
            if let Some(user) = users::Model::by_provider_identity(db, provider, &provider_user.subject)
                .await
                .map_err(|e| AuthError::internal_error(format!("Database error: {}", e)))?
            {
                span.record("auth.user_resolution", "matched_provider_subject");
                user.ensure_enabled()?;
                return Ok(user);
            }

            // Second, try to find user by email and link this provider.
            if let Some(mut user) = users::Model::find_by_email(db, &provider_user.email)
                .await
                .map_err(|e| AuthError::internal_error(format!("Database error: {}", e)))?
            {
                user.ensure_enabled()?;
                // Link provider account to existing user.
                user = user
                    .link_oauth_identity(db, provider, provider_user.subject)
                    .await
                    .map_err(|e| AuthError::internal_error(format!("Failed to update user: {}", e)))?;

                span.record("auth.user_resolution", "linked_existing_email");
                return Ok(user);
            }

            // Create new OAuth-only user.
            let user = users::Model::create_oauth_user(db, provider, provider_user)
                .await
                .map_err(|e| AuthError::internal_error(format!("Failed to create user: {}", e)))?;

            span.record("auth.user_resolution", "created_user");
            Ok(user)
        }
        .instrument(span.clone())
        .await
    }

    /// Build the synthetic provider user used by the explicitly-enabled local dev provider.
    pub fn local_dev_user_info() -> OAuthUserInfo {
        let email = env_or_default("OAUTH_DEV_EMAIL", "developer@bike.local");

        OAuthUserInfo {
            subject: env_or_default("OAUTH_DEV_SUBJECT", "local-dev"),
            email: email.clone(),
            name: Some(env_or_default("OAUTH_DEV_NAME", "Local Developer")),
            preferred_username: Some(email),
            verified_email: Some(true),
        }
    }
}

fn env_or_default(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}
