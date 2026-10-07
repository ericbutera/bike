use migration::{ConnectionTrait, Migrator, MigratorTrait};
use sea_orm_migration::sea_orm::Database;
use std::{env, error::Error, fs, path::PathBuf};

fn validate_database_url(url: &str) -> Result<(), Box<dyn Error>> {
    let allowed = ["ci-postgres", "127.0.0.1", "localhost"]
        .map(|host| format!("postgres://bike_e2e:bike-e2e-only@{host}:5432/bike_e2e"));
    if !allowed.iter().any(|allowed| allowed == url) {
        return Err("Fixture reset requires the dedicated bike_e2e database and role".into());
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let url = env::var("BIKE_E2E_DATABASE_URL")?;
    validate_database_url(&url)?;
    let db = Database::connect(url).await?;
    Migrator::fresh(&db).await?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../api/tests/fixtures/platform");
    db.execute_unprepared(&fs::read_to_string(root.join("four-views.sql"))?)
        .await?;
    for name in env::args().skip(1) {
        let path = root.join(&name);
        if path.parent() != Some(root.as_path()) || path.extension().is_none_or(|ext| ext != "sql")
        {
            return Err("Expected a platform SQL fixture filename".into());
        }
        db.execute_unprepared(&fs::read_to_string(path)?).await?;
    }
    db.close().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_database_url;

    #[test]
    fn reset_requires_a_dedicated_database_role_and_address() {
        for host in ["ci-postgres", "127.0.0.1", "localhost"] {
            assert!(validate_database_url(&format!(
                "postgres://bike_e2e:bike-e2e-only@{host}:5432/bike_e2e"
            ))
            .is_ok());
        }
        for url in [
            "postgres://postgres:postgres@localhost:5432/bike",
            "postgres://bike_e2e:bike-e2e-only@production:5432/bike_e2e",
            "postgres://bike_e2e:bike-e2e-only@localhost:5432/bike_e2e?options=-csearch_path=other",
        ] {
            assert!(validate_database_url(url).is_err());
        }
    }
}
