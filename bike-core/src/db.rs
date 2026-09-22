use sea_orm::{ConnectOptions, DatabaseConnection};
use std::time::Duration;

pub async fn connect_database(database_url: &str) -> Result<DatabaseConnection, sea_orm::DbErr> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Debug)
        .sqlx_slow_statements_logging_settings(log::LevelFilter::Warn, Duration::from_millis(250));

    sea_orm::Database::connect(options).await
}
