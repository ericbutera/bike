use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};

pub type Coordinates = Vec<[f64; 2]>;

pub fn wkb(points: &[[f64; 2]]) -> Vec<u8> {
    let mut bytes = vec![1];
    bytes.extend(2u32.to_le_bytes());
    bytes.extend((points.len() as u32).to_le_bytes());
    for point in points {
        for value in point {
            bytes.extend(value.to_le_bytes());
        }
    }
    bytes
}

pub fn decode_wkb(bytes: &[u8]) -> Result<Coordinates> {
    ensure!(
        bytes.len() >= 9 && bytes[0] == 1 && u32::from_le_bytes(bytes[1..5].try_into()?) == 2,
        "Expected little-endian LineString WKB"
    );
    let count = u32::from_le_bytes(bytes[5..9].try_into()?) as usize;
    ensure!(bytes.len() == 9 + 16 * count, "Invalid WKB length");
    Ok(bytes[9..]
        .chunks_exact(16)
        .map(|part| {
            [
                f64::from_le_bytes(part[..8].try_into().unwrap()),
                f64::from_le_bytes(part[8..].try_into().unwrap()),
            ]
        })
        .collect())
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn pixel(point: [f64; 2], bbox: [f64; 4], size: usize, gutter: usize) -> [f64; 2] {
    fn mercator(lat: f64) -> f64 {
        ((std::f64::consts::PI / 4.0) + lat.clamp(-85.05112878, 85.05112878).to_radians() / 2.0)
            .tan()
            .ln()
    }
    [
        (point[0] - bbox[0]) / (bbox[2] - bbox[0]) * size as f64 + gutter as f64,
        (mercator(bbox[3]) - mercator(point[1])) / (mercator(bbox[3]) - mercator(bbox[1]))
            * size as f64
            + gutter as f64,
    ]
}

fn clipped(a: [f64; 2], b: [f64; 2], limit: f64) -> Option<([f64; 2], [f64; 2])> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let mut first: f64 = 0.0;
    let mut last: f64 = 1.0;
    for (p, q) in [
        (-d[0], a[0]),
        (d[0], limit - a[0]),
        (-d[1], a[1]),
        (d[1], limit - a[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let ratio = q / p;
            if p < 0.0 {
                first = first.max(ratio);
            } else {
                last = last.min(ratio);
            }
            if first > last {
                return None;
            }
        }
    }
    Some((
        [a[0] + first * d[0], a[1] + first * d[1]],
        [a[0] + last * d[0], a[1] + last * d[1]],
    ))
}

pub fn heatmap(
    routes: &[Coordinates],
    bbox: [f64; 4],
    size: usize,
    gutter: usize,
) -> Result<(Vec<u8>, usize)> {
    let width = size + 2 * gutter;
    let mut counts = vec![0u32; width * width];
    for route in routes {
        let mut covered = vec![false; width * width];
        for pair in route.windows(2) {
            // Explicit initial prototype policy: break coordinate jumps over
            // approximately 5 km. Temporal-gap semantics are a later experiment.
            let dx = (pair[1][0] - pair[0][0]) * pair[0][1].to_radians().cos();
            let dy = pair[1][1] - pair[0][1];
            if dx.hypot(dy) * 111_320.0 > 5000.0 {
                continue;
            }
            let a = pixel(pair[0], bbox, size, gutter);
            let b = pixel(pair[1], bbox, size, gutter);
            if let Some((a, b)) = clipped(a, b, (width - 1) as f64) {
                let steps = ((b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil() as usize).max(1);
                for step in 0..=steps {
                    let t = step as f64 / steps as f64;
                    let x = (a[0] + t * (b[0] - a[0])).round() as usize;
                    let y = (a[1] + t * (b[1] - a[1])).round() as usize;
                    covered[y.min(width - 1) * width + x.min(width - 1)] = true;
                }
            }
        }
        for (count, cover) in counts.iter_mut().zip(covered) {
            if cover {
                *count += 1;
            }
        }
    }
    let covered = counts.iter().filter(|n| **n > 0).count();
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let count = counts[(y + gutter) * width + x + gutter];
            let p = (y * size + x) * 4;
            rgba[p] = 255;
            rgba[p + 1] = 40;
            rgba[p + 3] = if count == 0 {
                0
            } else {
                (35.0 + 45.0 * (count as f64).ln_1p()).min(255.0) as u8
            };
        }
    }
    let mut output = Vec::new();
    let mut encoder = png::Encoder::new(&mut output, size as u32, size as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgba)?;
    Ok((output, covered))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wkb_preserves_coordinates() {
        let route = vec![[0.0, 0.0], [-120.123456789, 40.987654321]];
        assert_eq!(decode_wkb(&wkb(&route)).unwrap(), route);
        assert!(decode_wkb(&[1, 2]).is_err());
    }
    #[test]
    fn projection_preserves_current_json_decoder_precision() {
        // Synthetic decimal reproduces the one-ULP discrepancy that invalidated
        // SQL-prepared map inputs. Do not round or silently change the oracle.
        let decimal = "-84.0000950294877744";
        let current: f64 = serde_json::from_str(decimal).unwrap();
        assert_ne!(current.to_bits(), decimal.parse::<f64>().unwrap().to_bits());
        let route = vec![[current, 42.0], [current, 42.001]];
        assert_eq!(decode_wkb(&wkb(&route)).unwrap(), route);
    }
    #[test]
    fn one_activity_loop_counts_once() {
        let route = vec![
            [0.001, 0.001],
            [0.002, 0.002],
            [0.001, 0.001],
            [0.002, 0.002],
        ];
        let one = heatmap(&[route.clone()], [0.0, 0.0, 0.003, 0.003], 32, 2).unwrap();
        let simple = heatmap(&[route[..2].to_vec()], [0.0, 0.0, 0.003, 0.003], 32, 2).unwrap();
        assert_eq!(one, simple);
        let two = heatmap(&[route.clone(), route], [0.0, 0.0, 0.003, 0.003], 32, 2).unwrap();
        assert_ne!(one.0, two.0);
        assert_eq!(one.1, two.1);
    }
    #[test]
    fn outside_line_is_empty_and_crossing_is_clipped() {
        assert_eq!(
            heatmap(
                &[vec![[0.01, 0.01], [0.02, 0.02]]],
                [0.0, 0.0, 0.003, 0.003],
                32,
                2
            )
            .unwrap()
            .1,
            0
        );
        assert!(
            heatmap(
                &[vec![[-0.001, 0.001], [0.004, 0.001]]],
                [0.0, 0.0, 0.003, 0.003],
                32,
                2
            )
            .unwrap()
            .1 > 0
        );
    }
}
