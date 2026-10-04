//! Bounded, local diagnostic: cargo run -p bike-core --example heatmap_preview -- route.json output.png
use bike_core::activity_data::{deserialize_derived_activity_data, StoredActivityDerivedData};
use bike_core::heatmaps::{
    geometry,
    raster::{Raster, Tile},
};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let stored: StoredActivityDerivedData = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let route = deserialize_derived_activity_data(Some(&stored)).route_points;
    let begin = Instant::now();
    let chunks = geometry::prepare(&route);
    let projection_ms = begin.elapsed().as_secs_f64() * 1000.0;
    let center = geometry::project(
        route[route.len() / 2].longitude,
        route[route.len() / 2].latitude,
    );
    for zoom in [7, 14] {
        let tile = Tile {
            z: zoom,
            x: (center[0] * 2f64.powi(i32::from(zoom))) as u32,
            y: (center[1] * 2f64.powi(i32::from(zoom))) as u32,
        };
        let begin = Instant::now();
        let mut raster = Raster::new(tile);
        for _ in 0..25 {
            for chunk in chunks
                .iter()
                .filter(|c| c.band == geometry::zoom_band(zoom))
            {
                raster.add_chunk(&geometry::decode(&chunk.points).unwrap());
            }
            raster.finish_activity();
        }
        let png = raster.png()?;
        std::fs::write(format!("{}-{zoom}.png", args[2]), &png)?;
        println!(
            "zoom={zoom} render_ms={:.2} png_bytes={}",
            begin.elapsed().as_secs_f64() * 1000.0,
            png.len()
        );
    }
    println!(
        "source_points={} chunks={} projection_bytes={} projection_ms={projection_ms:.2}",
        route.len(),
        chunks.len(),
        chunks.iter().map(|c| c.points.len()).sum::<usize>()
    );
    Ok(())
}
