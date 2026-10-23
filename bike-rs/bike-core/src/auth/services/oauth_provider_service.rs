use crate::auth::services::provider_settings;
use serde::Serialize;
use std::collections::BTreeSet;
use std::env;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct OAuthProviderMetadata {
    pub id: String,
    pub label: String,
}

pub struct OAuthProviderService;

impl OAuthProviderService {
    pub fn enabled_providers() -> Vec<OAuthProviderMetadata> {
        provider_ids()
            .into_iter()
            .filter(|provider| Self::is_provider_enabled(provider))
            .map(|provider| OAuthProviderMetadata {
                label: button_label(&provider),
                id: provider,
            })
            .collect()
    }

    pub fn is_any_provider_enabled() -> bool {
        !Self::enabled_providers().is_empty()
    }

    pub fn is_provider_enabled(provider: &str) -> bool {
        let Ok(provider) = provider_settings::normalize_provider_id(provider) else {
            return false;
        };

        if provider == provider_settings::PROVIDER_DEV {
            return truthy_env(&format!("{}ENABLED", provider_settings::env_prefix(&provider)));
        }

        has_credentials(&provider) && has_provider_endpoints(&provider)
    }
}

fn provider_ids() -> Vec<String> {
    let mut ordered = Vec::new();
    let mut seen_ids = BTreeSet::new();
    let mut seen_prefixes = BTreeSet::new();

    for provider in env_string("OAUTH_PROVIDERS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|provider| provider_settings::normalize_provider_id(provider).ok())
    {
        if seen_ids.insert(provider.clone()) {
            seen_prefixes.insert(provider_settings::env_prefix(&provider));
            ordered.push(provider);
        }
    }

    for provider in inferred_provider_ids() {
        let prefix = provider_settings::env_prefix(&provider);
        if seen_ids.insert(provider.clone()) && seen_prefixes.insert(prefix) {
            ordered.push(provider);
        }
    }

    ordered
}

fn inferred_provider_ids() -> Vec<String> {
    let mut providers = BTreeSet::new();

    for (key, _) in env::vars() {
        if key == "OAUTH_DEV_ENABLED" {
            providers.insert(provider_settings::PROVIDER_DEV.to_string());
            continue;
        }

        let Some(rest) = key.strip_prefix("OAUTH_") else {
            continue;
        };

        let Some(provider_env) = rest
            .strip_suffix("_CLIENT_ID")
            .or_else(|| rest.strip_suffix("_ISSUER_URL"))
            .or_else(|| rest.strip_suffix("_AUTH_URL"))
        else {
            continue;
        };

        if provider_env == "PROVIDERS" {
            continue;
        }

        let provider = provider_env.to_ascii_lowercase();
        if provider_settings::normalize_provider_id(&provider).is_ok() {
            providers.insert(provider);
        }
    }

    providers.into_iter().collect()
}

fn has_credentials(provider: &str) -> bool {
    let prefix = provider_settings::env_prefix(provider);

    has_env(&format!("{prefix}CLIENT_ID")) && has_env(&format!("{prefix}CLIENT_SECRET"))
}

fn has_provider_endpoints(provider: &str) -> bool {
    let prefix = provider_settings::env_prefix(provider);
    let has_explicit_endpoints = has_env(&format!("{prefix}AUTH_URL"))
        && has_env(&format!("{prefix}TOKEN_URL"))
        && has_env(&format!("{prefix}USERINFO_URL"));
    let has_issuer = has_env(&format!("{prefix}ISSUER_URL"));

    has_explicit_endpoints || has_issuer
}

fn button_label(provider: &str) -> String {
    let prefix = provider_settings::env_prefix(provider);

    if let Some(label) = env_string(&format!("{prefix}LABEL")) {
        return format!("Continue with {label}");
    }

    let label = if provider == provider_settings::PROVIDER_DEV {
        "local developer".to_string()
    } else {
        provider
            .split(['-', '_'])
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_ascii_lowercase(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };

    format!("Continue with {label}")
}

fn has_env(name: &str) -> bool {
    env_string(name).is_some()
}

fn env_string(name: &str) -> Option<String> {
    env::var(name)
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
}

fn truthy_env(name: &str) -> bool {
    env::var(name)
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}
