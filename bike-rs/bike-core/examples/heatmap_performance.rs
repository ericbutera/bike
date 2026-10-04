//! Read-only tile diagnostic against prepared native heatmap tables.
//! Args: <user-id> <private-cases.json from the existing PostGIS experiment>.
use bike_core::heatmaps::{geometry, raster::Tile, service::HeatmapService, types::HeatmapQuery};
use sea_orm::Database;
use serde::Deserialize;

#[derive(Deserialize)]
struct Cases {
    regions: Vec<Region>,
}
#[derive(Deserialize)]
struct Region {
    id: String,
    center: [f64; 2],
    activities: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let user_id: i32 = args
        .get(1)
        .ok_or("Supply user ID and cases path")?
        .parse()?;
    let cases: Cases =
        serde_json::from_slice(&std::fs::read(args.get(2).ok_or("Supply cases path")?)?)?;
    let db = Database::connect(std::env::var("DATABASE_URL")?).await?;
    let mut regions = cases.regions;
    regions.sort_by_key(|r| std::cmp::Reverse(r.activities));
    let mut selected = vec![("world".to_owned(), Tile { z: 0, x: 0, y: 0 })];
    for region in regions.iter().take(2) {
        let p = geometry::project(region.center[1], region.center[0]);
        for z in [7, 14, 18] {
            let n = 2f64.powi(i32::from(z));
            selected.push((
                format!("{}-z{z}", region.id),
                Tile {
                    z,
                    x: (p[0] * n) as u32,
                    y: (p[1] * n) as u32,
                },
            ));
        }
    }
    for (name, tile) in selected {
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        let mut bytes = 0;
        for _ in 0..5 {
            let service = HeatmapService::default();
            let metadata = service
                .metadata(&db, user_id, HeatmapQuery::default())
                .await?;
            let query = HeatmapQuery {
                revision: Some(metadata.revision),
                ..Default::default()
            };
            let start = std::time::Instant::now();
            let image = service.tile(&db, user_id, query.clone(), tile).await?;
            cold.push(start.elapsed().as_secs_f64() * 1000.0);
            bytes = image.png.len();
            let start = std::time::Instant::now();
            service.tile(&db, user_id, query, tile).await?;
            warm.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        cold.sort_by(f64::total_cmp);
        warm.sort_by(f64::total_cmp);
        println!(
            "case={name} cold_median_ms={:.3} warm_median_ms={:.3} png_bytes={bytes}",
            cold[2], warm[2]
        );
    }
    Ok(())
}
