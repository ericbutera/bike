use bike_core::activity_source_recovery::{
    apply_recovery, plan_recovery, refresh_retained_recording, register_archive_source,
    retire_generated_artifact, withhold_unrecovered_source, ArchivedActivitySource,
    RecoveryOutcome,
};
use bike_core::config::Config;
use bike_core::entities::{activity_import_artifacts, activity_imports};
use bike_core::strava_gateway_client::StravaGatewayClient;
use chrono::Utc;
use sea_orm::Database;
use std::error::Error;

struct RecoveryRun {
    user_id: i32,
    after_id: i32,
    limit: u64,
    apply: bool,
    refresh_recent: bool,
    cleanup: bool,
    register_manifest: Option<String>,
    withhold_unrecovered: bool,
    refresh_recording: bool,
}

impl RecoveryRun {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let mut args = args.peekable();
        let mut run = Self {
            user_id: 0,
            after_id: 0,
            limit: 32,
            apply: false,
            refresh_recent: false,
            cleanup: false,
            register_manifest: None,
            withhold_unrecovered: false,
            refresh_recording: false,
        };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--user-id" => run.user_id = args.next().ok_or("Missing user ID")?.parse()?,
                "--after-id" => run.after_id = args.next().ok_or("Missing cursor")?.parse()?,
                "--limit" => run.limit = args.next().ok_or("Missing batch limit")?.parse()?,
                "--apply" => run.apply = true,
                "--refresh-recent" => run.refresh_recent = true,
                "--cleanup" => run.cleanup = true,
                "--withhold-unrecovered" => run.withhold_unrecovered = true,
                "--refresh-recording" => run.refresh_recording = true,
                "--register-manifest" => {
                    run.register_manifest =
                        Some(args.next().ok_or("Missing archive manifest path")?)
                }
                _ => return Err(format!("Unknown recovery option: {arg}").into()),
            }
        }
        if run.user_id <= 0
            || run.after_id < 0
            || !(1..=32).contains(&run.limit)
            || (run.refresh_recent && !run.apply)
            || (run.cleanup && run.refresh_recent)
            || (run.register_manifest.is_some() && (run.cleanup || run.refresh_recent))
            || (run.withhold_unrecovered
                && (!run.apply || run.cleanup || run.register_manifest.is_some()))
            || (run.refresh_recording
                && (run.cleanup
                    || run.register_manifest.is_some()
                    || run.refresh_recent
                    || run.withhold_unrecovered))
        {
            return Err("Require --user-id > 0, --after-id >= 0, --limit 1..32; --refresh-recent requires --apply".into());
        }
        Ok(run)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let run = RecoveryRun::parse(std::env::args().skip(1))?;
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL")
        .map_err(|_| "Set DATABASE_URL for the intended recovery database")?;
    let config = Config::init_from_env();
    let db = Database::connect(&config.database_url).await?;
    if run.refresh_recording {
        return refresh_recording(&run, &db, &config.uploads_dir).await;
    }
    if let Some(path) = &run.register_manifest {
        return register_manifest(&run, &db, &config.uploads_dir, path).await;
    }
    if run.cleanup {
        return cleanup(&run, &db, &config.uploads_dir).await;
    }
    let now = Utc::now();
    let imports =
        activity_imports::Entity::generated_strava_page(&db, run.user_id, run.after_id, run.limit)
            .await?;
    let mut recent_gaps = false;
    let mut last_id = run.after_id;
    let mut failed = 0;
    for import in imports {
        last_id = import.id;
        let plan = match plan_recovery(&db, &config.uploads_dir, import, now).await {
            Ok(plan) => plan,
            Err(error) => {
                failed += 1;
                println!(
                    "{}",
                    serde_json::json!({"import_id": last_id, "error": error.to_string()})
                );
                continue;
            }
        };
        recent_gaps |= plan.outcome == RecoveryOutcome::RecentStravaRequired;
        println!("{}", serde_json::to_string(&plan)?);
        let reprocessing_required = plan.reprocessing_required;
        if run.apply {
            if run.withhold_unrecovered && plan.outcome == RecoveryOutcome::ArchiveRequired {
                let changed = withhold_unrecovered_source(&db, &plan).await?;
                println!(
                    "{}",
                    serde_json::json!({"import_id":last_id,"heatmap_withheld":changed})
                );
                continue;
            }
            match apply_recovery(&db, &config.uploads_dir, plan).await {
                Ok(promoted) => println!(
                    "{}",
                    serde_json::json!({"import_id": last_id, "source_promoted": promoted, "reprocessing_queued": promoted && reprocessing_required})
                ),
                Err(error) => {
                    failed += 1;
                    println!(
                        "{}",
                        serde_json::json!({"import_id": last_id, "error": error.to_string()})
                    );
                }
            }
        }
    }
    if run.refresh_recent && recent_gaps {
        StravaGatewayClient::from_config(config)?
            .queue_sync(run.user_id)
            .await?;
    }
    println!(
        "{}",
        serde_json::json!({"next_after_id": last_id, "applied": run.apply, "failed": failed, "recent_refresh_requested": run.refresh_recent && recent_gaps})
    );
    if failed > 0 {
        return Err(format!("{failed} recovery records failed; resolve and revisit those import IDs before considering backfill complete").into());
    }
    Ok(())
}

async fn cleanup(
    run: &RecoveryRun,
    db: &sea_orm::DatabaseConnection,
    uploads_dir: &str,
) -> Result<(), Box<dyn Error>> {
    let artifacts =
        activity_import_artifacts::Entity::generated_page(db, run.user_id, run.after_id, run.limit)
            .await?;
    let mut last_id = run.after_id;
    let mut failed = 0;
    for artifact in artifacts {
        last_id = artifact.id;
        match retire_generated_artifact(db, uploads_dir, artifact, run.apply).await {
            Ok(eligible) => println!(
                "{}",
                serde_json::json!({"artifact_id":last_id,"eligible":eligible,"deleted":eligible && run.apply})
            ),
            Err(error) => {
                failed += 1;
                println!(
                    "{}",
                    serde_json::json!({"artifact_id":last_id,"error":error.to_string()})
                );
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"next_after_id":last_id,"applied":run.apply,"failed":failed})
    );
    if failed > 0 {
        return Err(format!("{failed} cleanup records failed").into());
    }
    Ok(())
}

async fn register_manifest(
    run: &RecoveryRun,
    db: &sea_orm::DatabaseConnection,
    uploads_dir: &str,
    path: &str,
) -> Result<(), Box<dyn Error>> {
    use tokio::io::AsyncReadExt;
    let file = tokio::fs::File::open(path).await?;
    let mut bytes = Vec::new();
    file.take(1_048_577).read_to_end(&mut bytes).await?;
    if bytes.len() > 1_048_576 {
        return Err("Archive manifest exceeds 1 MiB".into());
    }
    let sources: Vec<ArchivedActivitySource> = serde_json::from_slice(&bytes)?;
    if sources.is_empty() || sources.len() > 32 {
        return Err("Archive manifest requires 1..32 entries".into());
    }
    for source in sources {
        let id = source.import_id;
        register_archive_source(db, uploads_dir, run.user_id, source, run.apply).await?;
        println!(
            "{}",
            serde_json::json!({"import_id":id,"validated":true,"registered":run.apply})
        );
    }
    Ok(())
}

async fn refresh_recording(
    run: &RecoveryRun,
    db: &sea_orm::DatabaseConnection,
    uploads_dir: &str,
) -> Result<(), Box<dyn Error>> {
    let activities = bike_core::entities::activities::Model::recording_page(
        db,
        run.user_id,
        run.after_id,
        run.limit,
    )
    .await?;
    let mut last_id = run.after_id;
    let mut failed = 0;
    for activity in activities {
        last_id = activity.id;
        match refresh_retained_recording(db, uploads_dir, activity, run.apply).await {
            Ok(changed) => println!(
                "{}",
                serde_json::json!({"activity_id":last_id,"evidence_changed":changed,"stored":changed && run.apply})
            ),
            Err(error) => {
                failed += 1;
                println!(
                    "{}",
                    serde_json::json!({"activity_id":last_id,"error":error.to_string()})
                );
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"next_after_id":last_id,"failed":failed,"applied":run.apply})
    );
    if failed > 0 {
        return Err(format!("{failed} recording refreshes failed").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_is_default_and_remote_refresh_requires_explicit_apply() {
        let run = RecoveryRun::parse(["--user-id", "4"].map(String::from).into_iter()).unwrap();
        assert!(!run.apply);
        assert!(!run.refresh_recent);
        for args in [
            vec!["--user-id", "4", "--refresh-recent"],
            vec!["--user-id", "4", "--limit", "33"],
            vec!["--user-id", "0"],
        ] {
            assert!(RecoveryRun::parse(args.into_iter().map(String::from)).is_err());
        }
    }
}
