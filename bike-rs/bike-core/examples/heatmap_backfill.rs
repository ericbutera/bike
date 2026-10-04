//! Explicit, bounded owner backfill; safe to rerun, including with serving disabled.
//! DATABASE_URL=... mise exec -- cargo run -p bike-core --example heatmap_backfill -- <user-id>
use bike_core::heatmaps::{preparation::prepare_activity, projection::Projection};
use sea_orm::Database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_id: i32 = std::env::args().nth(1).ok_or("Supply a user ID")?.parse()?;
    if user_id <= 0 {
        return Err("User ID must be positive".into());
    }
    let db = Database::connect(std::env::var("DATABASE_URL")?).await?;
    let start = std::time::Instant::now();
    let mut cursor = 0;
    let mut processed = 0;
    loop {
        let page = Projection::backfill_page(&db, user_id, cursor).await?;
        if page.is_empty() {
            break;
        }
        for pending in page {
            cursor = pending.activity_id;
            prepare_activity(&db, pending).await?;
            processed += 1;
        }
    }
    println!(
        "prepared={processed} elapsed_seconds={:.3}",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
