use super::geometry::{unproject, Point, WORLD_METERS};

pub const TILE_SIZE: usize = 512;

#[derive(Clone, Copy, Debug)]
pub struct Tile {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl Tile {
    pub fn is_valid(self) -> bool {
        self.z <= 18 && self.x < (1u32 << self.z) && self.y < (1u32 << self.z)
    }

    pub fn query_bounds(self) -> Vec<[f64; 4]> {
        let n = 2f64.powi(i32::from(self.z));
        let pad = self.layout().gutter as f64 / (TILE_SIZE as f64 * n);
        let b = [
            f64::from(self.x) / n - pad,
            (f64::from(self.y) / n - pad).max(0.0),
            (f64::from(self.x + 1) / n + pad),
            (f64::from(self.y + 1) / n + pad).min(1.0),
        ];
        let mut boxes = vec![[b[0].max(0.0), b[1], b[2].min(1.0), b[3]]];
        if b[0] < 0.0 {
            boxes.push([1.0 + b[0], b[1], 1.0, b[3]]);
        }
        if b[2] > 1.0 {
            boxes.push([0.0, b[1], b[2] - 1.0, b[3]]);
        }
        boxes
    }

    fn layout(self) -> RasterLayout {
        let n = 2f64.powi(i32::from(self.z));
        let latitude = unproject([0.5, (f64::from(self.y) + 0.5) / n])[1];
        let meters = WORLD_METERS * latitude.to_radians().cos() / (n * TILE_SIZE as f64);
        // Count on a coarser grid at high zoom, while painting original pixel
        // centerlines. GPS tolerance must not become a very wide painted halo.
        let mut scale = 1;
        while 5.0 / (meters * scale as f64) > 6.0 {
            scale *= 2;
        }
        let tolerance = (5.0 / (meters * scale as f64)).round().max(1.0) as i32;
        let gutter = ((tolerance as usize + 1) * scale + 8).max(16);
        let width = TILE_SIZE + 2 * gutter;
        RasterLayout {
            scale,
            tolerance,
            gutter,
            width,
            count_width: width.div_ceil(scale) + 1,
        }
    }

    fn pixel(self, point: Point, gutter: usize) -> Point {
        let n = 2f64.powi(i32::from(self.z));
        let center = (f64::from(self.x) + 0.5) / n;
        let wrapped_x = if self.z == 0 {
            point[0]
        } else {
            point[0] + (center - point[0]).round()
        };
        [
            (wrapped_x * n - f64::from(self.x)) * TILE_SIZE as f64 + gutter as f64,
            (point[1] * n - f64::from(self.y)) * TILE_SIZE as f64 + gutter as f64,
        ]
    }
}

/// The mask spans all chunks belonging to one activity. Finishing an activity
/// once prevents repeated laps and overlapping chunk endpoints from adding heat.
pub struct Raster {
    tile: Tile,
    layout: RasterLayout,
    counts: Vec<u32>,
    mask: Vec<bool>,
    ink: Vec<bool>,
    touched: Vec<usize>,
}

struct RasterLayout {
    scale: usize,
    tolerance: i32,
    gutter: usize,
    width: usize,
    count_width: usize,
}

impl Raster {
    pub fn new(tile: Tile) -> Self {
        let layout = tile.layout();
        let cells = layout.count_width * layout.count_width;
        let pixels = layout.width * layout.width;
        Self {
            tile,
            layout,
            counts: vec![0; cells],
            mask: vec![false; cells],
            ink: vec![false; pixels],
            touched: Vec::new(),
        }
    }

    pub fn add_chunk(&mut self, points: &[Point]) {
        for pair in points.windows(2) {
            self.add_segment(
                self.tile.pixel(pair[0], self.layout.gutter),
                self.tile.pixel(pair[1], self.layout.gutter),
            );
        }
    }

    fn add_segment(&mut self, a: Point, b: Point) {
        let Some((a, b)) = clip(a, b, self.layout.width) else {
            return;
        };
        self.paint_centerline(a, b);
        let scale = self.layout.scale as f64;
        let a = [a[0] / scale, a[1] / scale];
        let b = [b[0] / scale, b[1] / scale];
        let steps = (b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil().max(1.0) as usize;
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            let x = (a[0] + t * (b[0] - a[0])).round() as i32;
            let y = (a[1] + t * (b[1] - a[1])).round() as i32;
            for (px, py) in disk(x, y, self.layout.tolerance, self.layout.count_width) {
                let index = py * self.layout.count_width + px;
                if !self.mask[index] {
                    self.mask[index] = true;
                    self.touched.push(index);
                }
            }
        }
    }

    fn paint_centerline(&mut self, a: Point, b: Point) {
        let steps = (b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil().max(1.0) as usize;
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            let x = (a[0] + t * (b[0] - a[0])).round() as usize;
            let y = (a[1] + t * (b[1] - a[1])).round() as usize;
            self.ink
                [y.min(self.layout.width - 1) * self.layout.width + x.min(self.layout.width - 1)] =
                true;
        }
    }

    pub fn finish_activity(&mut self) {
        for index in self.touched.drain(..) {
            self.counts[index] = self.counts[index].saturating_add(1);
            self.mask[index] = false;
        }
    }

    pub fn png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let rgba = self.rgba();
        let mut output = Vec::new();
        let mut encoder = png::Encoder::new(&mut output, TILE_SIZE as u32, TILE_SIZE as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&rgba)?;
        Ok(output)
    }

    fn rgba(&self) -> Vec<u8> {
        let mut rgba = vec![0; TILE_SIZE * TILE_SIZE * 4];
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let px = x + self.layout.gutter;
                let py = y + self.layout.gutter;
                if !self.ink[py * self.layout.width + px] {
                    continue;
                }
                let count_x = (px as f64 / self.layout.scale as f64).round() as usize;
                let count_y = (py as f64 / self.layout.scale as f64).round() as usize;
                let count = self.counts[count_y * self.layout.count_width + count_x];
                if count == 0 {
                    continue;
                }
                rgba[(y * TILE_SIZE + x) * 4..(y * TILE_SIZE + x + 1) * 4].copy_from_slice(&[
                    0,
                    96,
                    223,
                    opacity(count),
                ]);
            }
        }
        rgba
    }
}

fn opacity(count: u32) -> u8 {
    match count {
        0 => 0,
        1 => 100,
        2..=4 => 155,
        5..=9 => 195,
        10..=24 => 225,
        _ => 255,
    }
}

fn disk(x: i32, y: i32, radius: i32, width: usize) -> impl Iterator<Item = (usize, usize)> {
    (-radius..=radius).flat_map(move |dy| {
        (-radius..=radius).filter_map(move |dx| {
            let px = x + dx;
            let py = y + dy;
            (dx * dx + dy * dy <= radius * radius
                && px >= 0
                && py >= 0
                && px < width as i32
                && py < width as i32)
                .then_some((px as usize, py as usize))
        })
    })
}

fn clip(a: Point, b: Point, width: usize) -> Option<(Point, Point)> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let mut first: f64 = 0.0;
    let mut last: f64 = 1.0;
    for (p, q) in [
        (-d[0], a[0]),
        (d[0], (width - 1) as f64 - a[0]),
        (-d[1], a[1]),
        (d[1], (width - 1) as f64 - a[1]),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gps_neighbors_share_counts_without_painting_a_wide_tolerance_halo() {
        let center = super::super::geometry::project(-85.6, 44.7);
        let n = 262_144.0;
        let tile = Tile {
            z: 18,
            x: (center[0] * n) as u32,
            y: (center[1] * n) as u32,
        };
        let route = [
            [(f64::from(tile.x) + 0.2) / n, (f64::from(tile.y) + 0.5) / n],
            [(f64::from(tile.x) + 0.8) / n, (f64::from(tile.y) + 0.5) / n],
        ];
        let offset = 3.0 / (WORLD_METERS * 44.7f64.to_radians().cos());
        let nearby = route.map(|p| [p[0], p[1] + offset]);
        let mut raster = Raster::new(tile);
        raster.add_chunk(&route);
        raster.finish_activity();
        raster.add_chunk(&nearby);
        raster.finish_activity();
        let image = raster.rgba();
        assert_eq!(image.chunks_exact(4).map(|p| p[3]).max(), Some(155));
        assert!(image.chunks_exact(4).filter(|p| p[3] > 0).count() < 4096);
        let mut separate = Raster::new(tile);
        separate.add_chunk(&route);
        separate.finish_activity();
        separate.add_chunk(&route.map(|p| [p[0], p[1] + offset * 5.0]));
        separate.finish_activity();
        assert_eq!(
            separate.rgba().chunks_exact(4).map(|p| p[3]).max(),
            Some(100)
        );
    }
    #[test]
    fn repeated_traversals_count_once_and_frequency_changes_only_blue_opacity() {
        let tile = Tile {
            z: 10,
            x: 250,
            y: 350,
        };
        let route = [
            [250.1 / 1024.0, 350.5 / 1024.0],
            [250.9 / 1024.0, 350.5 / 1024.0],
        ];
        let mut raster = Raster::new(tile);
        raster.add_chunk(&route);
        raster.add_chunk(&route);
        raster.finish_activity();
        let single = raster.rgba();
        assert_eq!(*raster.counts.iter().max().unwrap(), 1);
        for _ in 1..150 {
            raster.add_chunk(&route);
            raster.finish_activity();
        }
        let frequent = raster.rgba();
        assert_eq!(
            frequent.chunks_exact(4).filter(|p| p[3] > 0).count(),
            single.chunks_exact(4).filter(|p| p[3] > 0).count()
        );
        for (one, many) in single.chunks_exact(4).zip(frequent.chunks_exact(4)) {
            assert_eq!(one[3] > 0, many[3] > 0);
            if one[3] > 0 {
                assert_eq!(&one[..3], &[0, 96, 223]);
                assert_eq!(&many[..3], &one[..3]);
                assert_eq!(many[3], 255);
            }
        }
        assert!(
            frequent.chunks_exact(4).map(|p| p[3]).max()
                > single.chunks_exact(4).map(|p| p[3]).max()
        );
        assert_eq!(&raster.png().unwrap()[..8], b"\x89PNG\r\n\x1a\n");
    }
    #[test]
    fn adjacent_tile_edges_share_the_same_intensity() {
        let route = [
            [250.99 / 1024.0, 350.5 / 1024.0],
            [251.01 / 1024.0, 350.5 / 1024.0],
        ];
        let images: Vec<_> = [250, 251]
            .into_iter()
            .map(|x| {
                let mut raster = Raster::new(Tile { z: 10, x, y: 350 });
                for _ in 0..25 {
                    raster.add_chunk(&route);
                    raster.finish_activity();
                }
                raster.rgba()
            })
            .collect();
        for y in 0..TILE_SIZE {
            assert_eq!(
                &images[0][(y * TILE_SIZE + TILE_SIZE - 1) * 4..(y * TILE_SIZE + TILE_SIZE) * 4],
                &images[1][y * TILE_SIZE * 4..(y * TILE_SIZE + 1) * 4]
            );
        }
    }
}
