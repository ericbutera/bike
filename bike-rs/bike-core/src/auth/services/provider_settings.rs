use crate::auth::error::AuthError;
use std::env;
use tracing::Instrument;
use url::Url;

/// Known provider identifiers
pub const PROVIDER_DEV: &str = "dev";

const DEFAULT_SCOPES: &[&str] = &["openid", "email", "profile"];

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub provider: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub auth_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    pub scopes: Vec<String>,
    pub issuer_url: Option<String>,
}

pub async fn get_provider_config(name: &str, api_url: &str) -> Result<ProviderConfig, AuthError> {
    let provider = normalize_provider_id(name)?;
    let prefix = env_prefix(&provider);
    let redirect_url = format!(
        "{}/api/oauth/{}/callback",
        api_url.trim_end_matches('/'),
        provider
    );

    if provider == PROVIDER_DEV {
        return Err(AuthError::internal_error(
            "The dev SSO provider does not use external OAuth configuration",
        ));
    }

    let client_id = required_env(&format!("{prefix}CLIENT_ID"))?;
    let client_secret = required_env(&format!("{prefix}CLIENT_SECRET"))?;
    let endpoints = provider_endpoints(&prefix, &provider).await?;

    Ok(ProviderConfig {
        provider,
        client_id,
        client_secret,
        redirect_url,
        auth_url: endpoints.auth_url,
        token_url: endpoints.token_url,
        userinfo_url: endpoints.userinfo_url,
        scopes: scopes_for_prefix(&prefix),
        issuer_url: env_string(&format!("{prefix}ISSUER_URL")),
    })
}

#[derive(Debug)]
struct ProviderEndpoints {
    auth_url: String,
    token_url: String,
    userinfo_url: String,
}

pub fn normalize_provider_id(name: &str) -> Result<String, AuthError> {
    let value = name.trim().to_ascii_lowercase();

    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(AuthError::validation("Invalid OAuth provider id"));
    }

    Ok(value)
}

pub fn env_prefix(provider: &str) -> String {
    let env_name = provider
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();

    format!("OAUTH_{env_name}_")
}

pub fn scopes_for_provider(provider: &str) -> Vec<String> {
    scopes_for_prefix(&env_prefix(provider))
}

fn scopes_for_prefix(prefix: &str) -> Vec<String> {
    env_string(&format!("{prefix}SCOPES"))
        .map(|raw| {
            raw.split([',', ' '])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|scopes| !scopes.is_empty())
        .unwrap_or_else(|| {
            DEFAULT_SCOPES
                .iter()
                .map(|scope| (*scope).to_string())
                .collect()
        })
}

fn required_env(name: &str) -> Result<String, AuthError> {
    env::var(name)
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AuthError::internal_error(format!("Missing OAuth environment: {name}")))
}

fn env_string(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn provider_endpoints(prefix: &str, provider: &str) -> Result<ProviderEndpoints, AuthError> {
    let explicit = match (
        env_string(&format!("{prefix}AUTH_URL")),
        env_string(&format!("{prefix}TOKEN_URL")),
        env_string(&format!("{prefix}USERINFO_URL")),
    ) {
        (Some(auth_url), Some(token_url), Some(userinfo_url)) => Some(ProviderEndpoints {
            auth_url,
            token_url,
            userinfo_url,
        }),
        _ => None,
    };

    if let Some(endpoints) = explicit {
        return Ok(endpoints);
    }

    let issuer_env = format!("{prefix}ISSUER_URL");
    let issuer_url = env_string(&issuer_env)
        .ok_or_else(|| {
            AuthError::internal_error(format!(
                "Missing OAuth endpoint configuration for provider {provider}: set {issuer_env} or AUTH_URL/TOKEN_URL/USERINFO_URL"
            ))
        })?;

    discover_oidc_endpoints(&issuer_url).await
}

#[derive(serde::Deserialize)]
struct OidcDiscoveryDocument {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
}

async fn discover_oidc_endpoints(issuer_url: &str) -> Result<ProviderEndpoints, AuthError> {
    let mut url = Url::parse(issuer_url)
        .map_err(|err| AuthError::internal_error(format!("Invalid OIDC issuer URL: {err}")))?;
    let mut path = url.path().trim_end_matches('/').to_string();
    path.push_str("/.well-known/openid-configuration");
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);

    let span = tracing::info_span!(
        "oauth.oidc.discovery",
        server.address = url.host_str().unwrap_or_default(),
    );

    let document: OidcDiscoveryDocument = async {
        reqwest::Client::new()
            .get(url)
            .send()
            .await
            .map_err(|err| {
                AuthError::internal_error(format!("OIDC discovery request failed: {err}"))
            })?
            .error_for_status()
            .map_err(|err| AuthError::internal_error(format!("OIDC discovery failed: {err}")))?
            .json()
            .await
            .map_err(|err| {
                AuthError::internal_error(format!("Invalid OIDC discovery document: {err}"))
            })
    }
    .instrument(span)
    .await?;

    Ok(ProviderEndpoints {
        auth_url: document.authorization_endpoint,
        token_url: document.token_endpoint,
        userinfo_url: document.userinfo_endpoint,
    })
}
