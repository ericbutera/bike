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
            return truthy_env(&format!(
                "{}ENABLED",
                provider_settings::env_prefix(&provider)
            ));
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
                    Some(first) => {
                        first.to_uppercase().collect::<String>()
                            + &chars.as_str().to_ascii_lowercase()
                    }
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
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const EXTRA_ENV_KEYS: &[&str] = &[
        "DEV_SSO_ENABLED",
        "OIDC_CLIENT_ID",
        "OIDC_CLIENT_SECRET",
        "OIDC_ISSUER_URL",
    ];

    struct EnvSnapshot {
        values: BTreeMap<String, Option<String>>,
    }

    impl EnvSnapshot {
        fn clear_oauth_env() -> Self {
            let mut keys = env::vars()
                .map(|(key, _)| key)
                .filter(|key| key.starts_with("OAUTH_"))
                .collect::<Vec<_>>();

            keys.extend(EXTRA_ENV_KEYS.iter().map(|key| (*key).to_string()));
            keys.sort();
            keys.dedup();

            let values = keys
                .iter()
                .map(|key| (key.clone(), env::var(key).ok()))
                .collect::<BTreeMap<_, _>>();

            for key in keys {
                env::remove_var(key);
            }

            Self { values }
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }

    fn with_clean_oauth_env(test: impl FnOnce()) {
        let _lock = ENV_LOCK.lock().expect("env test lock poisoned");
        let _snapshot = EnvSnapshot::clear_oauth_env();

        test();
    }

    fn provider_ids_for_test() -> Vec<String> {
        OAuthProviderService::enabled_providers()
            .into_iter()
            .map(|provider| provider.id)
            .collect()
    }

    #[test]
    fn ignores_unprefixed_provider_env_names() {
        with_clean_oauth_env(|| {
            env::set_var("OIDC_CLIENT_ID", "client");
            env::set_var("OIDC_CLIENT_SECRET", "secret");
            env::set_var("OIDC_ISSUER_URL", "https://idp.example.com");
            env::set_var("DEV_SSO_ENABLED", "true");

            assert!(OAuthProviderService::enabled_providers().is_empty());
            assert!(!OAuthProviderService::is_provider_enabled("sso"));
            assert!(!OAuthProviderService::is_provider_enabled("dev"));
        });
    }

    #[test]
    fn exposes_complete_oauth_prefixed_providers_in_configured_order() {
        with_clean_oauth_env(|| {
            env::set_var("OAUTH_PROVIDERS", "acme,dev,internal");
            env::set_var("OAUTH_ACME_CLIENT_ID", "acme-client");
            env::set_var("OAUTH_ACME_CLIENT_SECRET", "acme-secret");
            env::set_var("OAUTH_ACME_ISSUER_URL", "https://idp.example.com");
            env::set_var("OAUTH_DEV_ENABLED", "true");
            env::set_var("OAUTH_INTERNAL_AUTH_URL", "https://login.example.com/auth");
            env::set_var(
                "OAUTH_INTERNAL_TOKEN_URL",
                "https://login.example.com/token",
            );
            env::set_var(
                "OAUTH_INTERNAL_USERINFO_URL",
                "https://login.example.com/userinfo",
            );
            env::set_var("OAUTH_INTERNAL_CLIENT_ID", "internal-client");
            env::set_var("OAUTH_INTERNAL_CLIENT_SECRET", "internal-secret");

            assert_eq!(provider_ids_for_test(), vec!["acme", "dev", "internal"]);
        });
    }

    #[test]
    fn filters_incomplete_inferred_providers() {
        with_clean_oauth_env(|| {
            env::set_var("OAUTH_ACME_CLIENT_ID", "acme-client");
            env::set_var("OAUTH_SSO_CLIENT_ID", "sso-client");
            env::set_var("OAUTH_SSO_ISSUER_URL", "https://idp.example.com");

            assert!(OAuthProviderService::enabled_providers().is_empty());

            env::set_var("OAUTH_SSO_CLIENT_SECRET", "sso-secret");

            assert_eq!(provider_ids_for_test(), vec!["sso"]);
        });
    }
}
