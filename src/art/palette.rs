//! Reducing true colour to the 256-colour palette: median cut picks the colours, and the
//! quantiser maps every painted pixel to one through a 4x4 ordered dither.

use super::FIRST_FREE;

/// Ordered-dither threshold for pixel `(x, y)`: the 4x4 Bayer matrix, tiled over the screen,
/// as 16 evenly spaced values in 0..1 (`(m + 0.5) / 16`, never exactly 0 or 1).
pub(super) fn bayer(x: usize, y: usize) -> f32 {
    const MATRIX: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (MATRIX[y % 4][x % 4] as f32 + 0.5) / 16.0
}

/// Nearest palette colour after a 4x4 Bayer nudge, cached on a 6-bit RGB grid.
pub struct Quantizer {
    /// The palette being matched, RGB 0..255. Entries below `FIRST_FREE` are never chosen.
    pub pal: [[u8; 3]; 256],
    /// Palette index per 6-bit-per-channel RGB cell (`2^18` cells); 0 means not computed yet
    /// (index 0 is the transparent key and is never a match, so it can serve as the marker).
    cache: Vec<u8>,
}

/// Peak-to-peak dither nudge in 0..255 units (about ±6 per channel).
const DITHER_SPREAD: f32 = 12.0;

impl Quantizer {
    /// A quantiser for `pal` with an empty cache.
    pub(super) fn new(pal: [[u8; 3]; 256]) -> Self {
        Self {
            pal,
            cache: vec![0; 1 << 18],
        }
    }

    /// Palette index for `colour` (RGB 0..255, out-of-range values clamped) at screen pixel
    /// `(x, y)`.
    ///
    /// 1. Nudge all channels by the Bayer threshold at `(x, y)`, centred on 0 and scaled to
    ///    ±`DITHER_SPREAD / 2`, so flat areas dither between neighbouring entries.
    /// 2. Reduce to a 6-bit-per-channel cell and use it as the cache key.
    /// 3. On a miss, find the free palette entry nearest the cell centre by weighted squared
    ///    distance (green 4, blue 3, red 2: roughly the eye's sensitivity) and cache it.
    pub fn get(&mut self, colour: [f32; 3], x: usize, y: usize) -> u8 {
        let nudge = (bayer(x, y) - 0.5) * DITHER_SPREAD;
        let cell = colour.map(|v| ((v + nudge).clamp(0.0, 255.0) as usize) >> 2);
        let key = cell[0] << 12 | cell[1] << 6 | cell[2];

        if self.cache[key] == 0 {
            // Centre of the 4-wide cell back in 0..255.
            let cell_centre = cell.map(|v| (v << 2) as f32 + 2.0);
            let mut best = (f32::MAX, 1);
            // Skip the transparent key and the UI colours.
            for (i, entry) in self.pal.iter().enumerate().skip(FIRST_FREE) {
                let (dr, dg, db) = (
                    cell_centre[0] - entry[0] as f32,
                    cell_centre[1] - entry[1] as f32,
                    cell_centre[2] - entry[2] as f32,
                );
                let distance = 2.0 * dr * dr + 4.0 * dg * dg + 3.0 * db * db;
                if distance < best.0 {
                    best = (distance, i);
                }
            }
            self.cache[key] = best.1 as u8;
        }
        self.cache[key]
    }
}

/// Reduce `samples` to at most `count` representative colours (median cut).
///
/// 1. Start with one box holding every sample.
/// 2. Until there are `count` boxes: pick the box (with more than one sample) whose widest
///    channel range, times the square root of its population, is largest; sort it along that
///    channel and split it at the median. Stop early if no box can be split.
/// 3. Each box becomes the mean colour of its samples.
///
/// Ties go to the last maximum, as `max_by` does; the result depends on sample order.
pub(super) fn median_cut(samples: Vec<[u8; 3]>, count: usize) -> Vec<[u8; 3]> {
    let mut boxes = vec![samples];
    while boxes.len() < count {
        // Split the box with the widest channel range (weighted by population).
        let Some((box_index, channel, _)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, bucket)| bucket.len() > 1)
            .map(|(i, bucket)| {
                let (channel, range) = (0..3)
                    .map(|channel| {
                        let (lo, hi) = bucket.iter().fold((255u8, 0u8), |(lo, hi), p| {
                            (lo.min(p[channel]), hi.max(p[channel]))
                        });
                        (channel, (hi - lo) as f32)
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .unwrap();
                (i, channel, range * (bucket.len() as f32).sqrt())
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
        else {
            break;
        };

        let mut lower = boxes.swap_remove(box_index);
        lower.sort_unstable_by_key(|p| p[channel]);
        let upper = lower.split_off(lower.len() / 2);
        boxes.push(lower);
        boxes.push(upper);
    }

    boxes
        .iter()
        .map(|bucket| {
            let sum = bucket.iter().fold([0u64; 3], |sum, p| {
                [
                    sum[0] + p[0] as u64,
                    sum[1] + p[1] as u64,
                    sum[2] + p[2] as u64,
                ]
            });
            sum.map(|v| (v / bucket.len().max(1) as u64) as u8)
        })
        .collect()
}
