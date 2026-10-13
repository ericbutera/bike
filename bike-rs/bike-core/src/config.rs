use once_cell::sync::OnceCell;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: String,
    pub database_url: String,
    pub frontend_url: String,
    pub cors_allowed_origins: Vec<String>,
    pub api_url: String,
    pub grafana_url: Option<String>,
    pub strava_client_id: String,
    pub strava_client_secret: String,
    pub strava_oauth_scopes: String,
    pub strava_webhook_verify_token: String,
    pub worker_recovery_timeout_seconds: i64,
    pub uploads_dir: String,
    pub map_renderer_grpc_address: Option<String>,
    pub map_service_token: String,
    pub map_image_cache_dir: String,
    pub map_image_cache_ttl_seconds: u64,
    pub max_upload_bytes: usize,
    pub max_archive_fetch_bytes: usize,
    pub archive_fetch_timeout_seconds: u64,
    pub jwt_secret: String,
    pub local_admin_enabled: bool,
    pub app_name: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from_email: String,
    pub smtp_from_name: String,
}

static CONFIG: OnceCell<Config> = OnceCell::new();

impl Config {
    pub fn init_from_env() -> &'static Config {
        dotenvy::dotenv().ok();

        let environment = env::var("APP_ENV").unwrap_or_else(|_| "local".to_string());
        let local_admin_enabled = env_bool("LOCAL_ADMIN_ENABLED", !is_production(&environment));
        assert!(
            !(local_admin_enabled && is_production(&environment)),
            "LOCAL_ADMIN_ENABLED must be false in production"
        );

        let cfg = Config {
            environment,
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/app".to_string()),
            frontend_url: env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
            cors_allowed_origins: env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:5173,http://localhost:3001".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            api_url: env::var("API_URL").unwrap_or_else(|_| "http://localhost:3000".to_string()),
            grafana_url: env::var("GRAFANA_URL").ok().filter(|value| {
                url::Url::parse(value).is_ok_and(|url| ["https", "http"].contains(&url.scheme()))
            }),
            strava_client_id: env::var("STRAVA_CLIENT_ID").unwrap_or_default(),
            strava_client_secret: env::var("STRAVA_CLIENT_SECRET").unwrap_or_default(),
            strava_oauth_scopes: env::var("STRAVA_OAUTH_SCOPES")
                .unwrap_or_else(|_| "activity:read_all".to_string()),
            strava_webhook_verify_token: env::var("STRAVA_WEBHOOK_VERIFY_TOKEN")
                .unwrap_or_default(),
            worker_recovery_timeout_seconds: worker_recovery_timeout(
                env::var("WORKER_RECOVERY_TIMEOUT_SECONDS")
                    .ok()
                    .filter(|value| !value.is_empty())
                    .as_deref(),
            ),
            uploads_dir: env::var("UPLOADS_DIR").unwrap_or_else(|_| "./uploads".to_string()),
            map_renderer_grpc_address: env_endpoint("MAP_RENDERER_GRPC_ADDRESS"),
            map_service_token: env::var("MAP_SERVICE_TOKEN").unwrap_or_default(),
            map_image_cache_dir: env::var("MAP_IMAGE_CACHE_DIR")
                .unwrap_or_else(|_| "./map-image-cache".into()),
            map_image_cache_ttl_seconds: env::var("MAP_IMAGE_CACHE_TTL_SECONDS")
                .unwrap_or_else(|_| "604800".into())
                .parse()
                .expect("MAP_IMAGE_CACHE_TTL_SECONDS must be a positive integer"),
            max_upload_bytes: env::var("MAX_UPLOAD_BYTES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(25 * 1024 * 1024),
            max_archive_fetch_bytes: env::var("MAX_ARCHIVE_FETCH_BYTES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(512 * 1024 * 1024),
            archive_fetch_timeout_seconds: env::var("ARCHIVE_FETCH_TIMEOUT_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(600),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "change_me_in_dev".to_string()),
            local_admin_enabled,
            app_name: env::var("APP_NAME").unwrap_or_else(|_| "App".to_string()),
            smtp_host: env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".to_string()),
            smtp_port: env::var("SMTP_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1025),
            smtp_username: env::var("SMTP_USER").ok(),
            smtp_password: env::var("SMTP_PASS").ok(),
            smtp_from_email: env::var("MAIL_FROM")
                .unwrap_or_else(|_| "noreply@app.local".to_string()),
            smtp_from_name: env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "App".to_string()),
        };

        CONFIG.get_or_init(|| cfg)
    }

    pub fn get() -> &'static Config {
        Self::init_from_env()
    }

    pub fn strava_enabled(&self) -> bool {
        !self.strava_client_id.trim().is_empty() && !self.strava_client_secret.trim().is_empty()
    }

    pub fn strava_webhook_enabled(&self) -> bool {
        self.strava_enabled() && !self.strava_webhook_verify_token.trim().is_empty()
    }

    pub fn strava_oauth_scope_list(&self) -> Vec<String> {
        self.strava_oauth_scopes
            .split([',', ' '])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }
}

fn env_endpoint(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

fn worker_recovery_timeout(value: Option<&str>) -> i64 {
    let seconds = value
        .unwrap_or("300")
        .parse::<u32>()
        .expect("WORKER_RECOVERY_TIMEOUT_SECONDS must be a positive integer");
    assert!(
        seconds > 0,
        "WORKER_RECOVERY_TIMEOUT_SECONDS must be a positive integer"
    );
    i64::from(seconds)
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        })
        .unwrap_or(default)
}

fn is_production(environment: &str) -> bool {
    matches!(
        environment.trim().to_ascii_lowercase().as_str(),
        "production" | "prod"
    )
}

#[cfg(test)]
mod recovery_timeout_tests {
    use super::worker_recovery_timeout;

    #[test]
    fn shared_recovery_timeout_uses_its_default_and_override() {
        assert_eq!(worker_recovery_timeout(None), 300);
        assert_eq!(worker_recovery_timeout(Some("120")), 120);
    }

    #[test]
    fn invalid_recovery_timeouts_are_rejected() {
        for value in ["0", "-1", "invalid", "9223372036854775808"] {
            assert!(std::panic::catch_unwind(|| worker_recovery_timeout(Some(value))).is_err());
        }
    }
}
