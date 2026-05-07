use once_cell::sync::OnceCell;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub frontend_url: String,
    pub cors_allowed_origins: Vec<String>,
    pub api_url: String,
    pub uploads_dir: String,
    pub max_upload_bytes: usize,
    pub jwt_secret: String,
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
            uploads_dir: env::var("UPLOADS_DIR").unwrap_or_else(|_| "./uploads".to_string()),
            max_upload_bytes: env::var("MAX_UPLOAD_BYTES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(25 * 1024 * 1024),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "change_me_in_dev".to_string()),
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
}
