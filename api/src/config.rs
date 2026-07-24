use once_cell::sync::OnceCell;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub frontend_url: String,
    pub cors_allowed_origins: Vec<String>,
    pub api_url: String,
    pub strava_client_id: String,
    pub strava_client_secret: String,
    pub strava_oauth_scopes: String,
    pub strava_webhook_verify_token: String,
    pub strava_webhook_callback_url: Option<String>,
    pub uploads_dir: String,
    pub max_upload_bytes: usize,
    pub max_archive_fetch_bytes: usize,
    pub archive_fetch_timeout_seconds: u64,
    pub jwt_secret: String,
    pub auth_password_enabled: bool,
    pub auth_registration_enabled: bool,
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

        let cfg = Config {
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
            strava_client_id: env::var("STRAVA_CLIENT_ID").unwrap_or_default(),
            strava_client_secret: env::var("STRAVA_CLIENT_SECRET").unwrap_or_default(),
            strava_oauth_scopes: env::var("STRAVA_OAUTH_SCOPES")
                .unwrap_or_else(|_| "activity:read_all".to_string()),
            strava_webhook_verify_token: env::var("STRAVA_WEBHOOK_VERIFY_TOKEN")
                .unwrap_or_default(),
            strava_webhook_callback_url: env::var("STRAVA_WEBHOOK_CALLBACK_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            uploads_dir: env::var("UPLOADS_DIR").unwrap_or_else(|_| "./uploads".to_string()),
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
            auth_password_enabled: env_bool("AUTH_PASSWORD_ENABLED", true),
            auth_registration_enabled: env_bool("AUTH_REGISTRATION_ENABLED", true),
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
        CONFIG.get_or_init(|| Self::init_from_env().clone())
    }

    pub fn strava_enabled(&self) -> bool {
        !self.strava_client_id.trim().is_empty() && !self.strava_client_secret.trim().is_empty()
    }

    pub fn strava_webhook_enabled(&self) -> bool {
        self.strava_enabled() && !self.strava_webhook_verify_token.trim().is_empty()
    }

    pub fn strava_webhook_callback_url(&self) -> String {
        self.strava_webhook_callback_url
            .clone()
            .unwrap_or_else(|| format!("{}/api/strava/webhook", self.api_url.trim_end_matches('/')))
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
