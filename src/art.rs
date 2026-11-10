//! All graphics are painted procedurally in true colour, then reduced to one 256-colour VGA
//! palette (median cut) with an ordered dither, the way 1990s artists prepared scanned art.

use game_toolkit_dosvideo::prelude::Bitmap;

pub mod dymo;
mod fridge;
pub mod slime;
mod spores;

use fridge::paint_fridge;
use spores::{penicillium, penicillium_neck, rhizopus, rhizopus_neck};

pub const W: usize = 640;
pub const H: usize = 480;
/// Hex cell circumradius (pointy-top cells).
pub const R: f32 = 30.0;
pub const BX: f32 = 250.0;
pub const BY: f32 = 240.0;
const SQ3: f32 = 1.732_050_8;
/// Colony sprites are `SPR x SPR`, centred on the cell.
pub const SPR: usize = 64;
pub const STAGES: usize = 10;
pub const VARIANTS: usize = 4;

/// Fixed palette entries; 0 doubles as the sprites' transparent key.
pub const TRANSPARENT: u8 = 0;
pub const INK: u8 = 1;
pub const GLOW_NEAR: u8 = 2;
pub const GLOW_FAR: u8 = 3;
pub const WHITE: u8 = 4;
const FIXED: [[u8; 3]; 5] = [
    [0, 0, 0],
    [38, 30, 52],
    [120, 255, 90],
    [255, 214, 60],
    [252, 252, 244],
];
const FIRST_FREE: usize = FIXED.len();

/// The fridge bulb, top right.
const BULB: (f32, f32) = (586.0, 18.0);

/// HUD column on the right-hand wall: left edge, width, and the centre every strip hangs on.
pub const HUD_X: i32 = 496;
pub const HUD_W: usize = 140;
pub const HUD_CX: i32 = HUD_X + HUD_W as i32 / 2;
/// Strip centres (y) for the title, each side's name, the first message line and the keys.
pub const TITLE_Y: i32 = 54;
pub const SIDE_Y: [i32; 2] = [102, 216];
pub const MESSAGE_Y: i32 = 336;
pub const MESSAGE_STEP: i32 = 27;
pub const KEYS_Y: i32 = 442;
pub const GAME_TITLE: &str = "SPORAGON";
pub const SIDE_NAMES: [&str; 2] = ["PENICILLIUM", "RHIZOPUS"];
pub const KEYS: &str = "N NEW D DEMO";

pub fn cell_center(q: i32, r: i32) -> (f32, f32) {
    (
        BX + R * SQ3 * (q as f32 + r as f32 / 2.0),
        BY + R * 1.5 * r as f32,
    )
}

/// The axial cell under a pixel (may lie off the board).
pub fn pixel_to_cell(x: f32, y: f32) -> (i32, i32) {
    let (px, py) = ((x - BX) / R, (y - BY) / R);
    let q = SQ3 / 3.0 * px - py / 3.0;
    let r = 2.0 / 3.0 * py;
    // Cube rounding.
    let s = -q - r;
    let (mut rq, mut rr, rs) = (q.round(), r.round(), s.round());
    let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());
    if dq > dr && dq > ds {
        rq = -rr - rs;
    } else if dr > ds {
        rr = -rq - rs;
    }
    (rq as i32, rr as i32)
}

/// 0 at the cell centre, 1 on its edge (hexagonal distance).
fn hex_norm(dx: f32, dy: f32) -> f32 {
    let a = dx.abs();
    let b = (dx * 0.5 + dy * SQ3 / 2.0).abs();
    let c = (dx * 0.5 - dy * SQ3 / 2.0).abs();
    a.max(b).max(c) / (R * SQ3 / 2.0)
}

// ------------------------------------------------------------------------------------------
// Noise

pub(crate) fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xFF_FFFF) as f32 / 16_777_216.0
}

pub(crate) fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (ix, iy) = (x0 as i32, y0 as i32);
    let a = hash(ix, iy, seed);
    let b = hash(ix + 1, iy, seed);
    let c = hash(ix, iy + 1, seed);
    let d = hash(ix + 1, iy + 1, seed);
    lerp(lerp(a, b, sx), lerp(c, d, sx), sy)
}

pub(crate) fn fbm(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, 1.0);
    for o in 0..octaves {
        sum += amp * vnoise(x * f, y * f, seed + o * 101);
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / norm
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub(crate) fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

pub(crate) fn scale(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// Direction to the bulb: top right, in front of the tray (screen space, y down).
const LIGHT: [f32; 3] = [0.45, -0.6, 0.66];

/// Lambert term for normal `n` (not clamped).
pub(crate) fn facing(n: [f32; 3]) -> f32 {
    n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2]
}

/// Blinn term for normal `n` with the viewer straight above (not clamped).
pub(crate) fn glint(n: [f32; 3]) -> f32 {
    // |LIGHT + (0, 0, 1)|
    const HALF_LEN: f32 = 1.8216;
    (facing(n) + n[2]) / HALF_LEN
}

pub(crate) fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Small deterministic RNG for scattering things.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }

    pub fn f(&mut self) -> f32 {
        (self.next() >> 8) as f32 / 16_777_216.0
    }

    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

// ------------------------------------------------------------------------------------------
// Colonies

/// RGBA, colour in 0..255, alpha 0..1.
/// A square true-colour sprite with coverage.
pub struct Rgba {
    pub w: usize,
    pub px: Vec<[f32; 4]>,
}

impl Rgba {
    pub fn new(w: usize) -> Self {
        Self {
            w,
            px: vec![[0.0; 4]; w * w],
        }
    }

    /// Paint `c` over the pixel with coverage `a`.
    pub fn over(&mut self, x: i32, y: i32, c: [f32; 3], a: f32) {
        let w = self.w as i32;
        if x < 0 || y < 0 || x >= w || y >= w || a <= 0.0 {
            return;
        }
        let p = &mut self.px[(y * w + x) as usize];
        let a = a.min(1.0);
        let na = a + p[3] * (1.0 - a);
        for i in 0..3 {
            p[i] = (c[i] * a + p[i] * p[3] * (1.0 - a)) / na.max(1e-6);
        }
        p[3] = na;
    }

    fn disk(&mut self, cx: f32, cy: f32, r: f32, c: [f32; 3], a: f32) {
        let (x0, x1) = ((cx - r - 1.0) as i32, (cx + r + 1.0) as i32);
        let (y0, y1) = ((cy - r - 1.0) as i32, (cy + r + 1.0) as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                self.over(x, y, c, a * (r + 0.5 - d).clamp(0.0, 1.0));
            }
        }
    }
}

pub struct Scene {
    pub background: Bitmap,
    /// `[side][variant][stage]`.
    pub colonies: [Vec<Vec<Bitmap>>; 2],
    /// Mucus under the colonies, translucent: `[side][variant][stage]`, `SPR` square.
    pub puddles: [Vec<Vec<Rgba>>; 2],
    /// Slime bridges between neighbours: `[side][direction][variant]`, `BRIDGE` square,
    /// centred on the midpoint between the cells. Directions as [`BRIDGE_DIRS`].
    pub bridges: [[Vec<Rgba>; 3]; 2],
    /// Mould grown across the ridge between neighbours, drawn over both colonies so they read
    /// as one mass. Same layout as `bridges`, each `(rim, core)`.
    pub necks: [[Vec<(Rgba, Rgba)>; 3]; 2],
    pub palette: [[u8; 3]; 256],
    pub quant: Quantizer,
}

pub const BRIDGE: usize = 80;
pub const BRIDGE_VARIANTS: usize = 3;
/// Axial offsets of the neighbours bridges are drawn toward (the other three are the same
/// bridges seen from the far end).
pub const BRIDGE_DIRS: [(i32, i32); 3] = [(1, 0), (1, -1), (0, -1)];

/// Paint everything. `centers` are the board cells, `true` for a knocked-out hole.
pub fn paint(centers: &[(f32, f32, bool)]) -> Scene {
    let sprites: [Vec<Vec<Rgba>>; 2] = [0u32, 1].map(|side| {
        (0..VARIANTS as u32)
            .map(|v| {
                (0..STAGES)
                    .map(|s| {
                        let g = (s + 1) as f32 / STAGES as f32;
                        if side == 0 {
                            penicillium(v * 31 + 1, g)
                        } else {
                            rhizopus(v * 37 + 2, g)
                        }
                    })
                    .collect()
            })
            .collect()
    });
    let mut bg = paint_fridge(centers);
    // The labels that never change are stuck on the wall for good.
    let tapes = [dymo::GREEN, dymo::GREY];
    for (text, tape, y) in [
        (GAME_TITLE, dymo::RED, TITLE_Y),
        (KEYS, dymo::BLACK, KEYS_Y),
    ]
    .into_iter()
    .chain((0..2).map(|s| (SIDE_NAMES[s], tapes[s], SIDE_Y[s])))
    {
        dymo::bake(&mut bg, W, &dymo::strip(text, tape, 2, y as u32), HUD_CX, y);
    }
    let puddles: [Vec<Vec<Rgba>>; 2] = [0usize, 1].map(|side| {
        (0..VARIANTS as u32)
            .map(|v| {
                (0..STAGES)
                    .map(|s| {
                        let g = (s + 1) as f32 / STAGES as f32;
                        slime::to_rgba(&slime::puddle(v * 13 + side as u32, g, SPR), side, SPR)
                    })
                    .collect()
            })
            .collect()
    });
    let bridges: [[Vec<Rgba>; 3]; 2] = [0usize, 1].map(|side| {
        BRIDGE_DIRS.map(|(q, r)| {
            let (x0, y0) = cell_center(0, 0);
            let (x1, y1) = cell_center(q, r);
            (0..BRIDGE_VARIANTS as u32)
                .map(|v| {
                    slime::to_rgba(
                        &slime::bridge((x1 - x0, y1 - y0), v * 7 + side as u32, BRIDGE),
                        side,
                        BRIDGE,
                    )
                })
                .collect()
        })
    });
    let necks: [[Vec<(Rgba, Rgba)>; 3]; 2] = [0usize, 1].map(|side| {
        BRIDGE_DIRS.map(|(q, r)| {
            let (x0, y0) = cell_center(0, 0);
            let (x1, y1) = cell_center(q, r);
            let v = (x1 - x0, y1 - y0);
            (0..BRIDGE_VARIANTS as u32)
                .map(|k| {
                    let seed = k * 19 + 3;
                    if side == 0 {
                        penicillium_neck(v, seed, BRIDGE)
                    } else {
                        rhizopus_neck(v, seed, BRIDGE)
                    }
                })
                .collect()
        })
    });

    let mut samples: Vec<[u8; 3]> = bg
        .iter()
        .step_by(3)
        .map(|c| c.map(|v| v.clamp(0.0, 255.0) as u8))
        .collect();
    // The bulb's rusty cage is small but catches the eye: weight it so it keeps its browns.
    for y in 0..40 {
        for x in (BULB.0 as usize - 24)..(BULB.0 as usize + 24) {
            samples.extend([bg[y * W + x].map(|v| v.clamp(0.0, 255.0) as u8); 6]);
        }
    }
    for side in &sprites {
        for variant in side {
            for img in variant.iter().skip(STAGES - 3) {
                // Weight the mould: it is what the player looks at.
                for p in img.px.iter().filter(|p| p[3] > 0.5) {
                    let c = [p[0], p[1], p[2]].map(|v| v.clamp(0.0, 255.0) as u8);
                    samples.extend([c; 3]);
                }
            }
        }
    }
    // Slime is translucent: sample it over clean and grimy plastic, as it will be seen.
    let slimes = puddles
        .iter()
        .flatten()
        .flat_map(|v| v.iter().skip(STAGES - 2));
    let neck_layers = necks.iter().flatten().flatten().flat_map(|(a, b)| [a, b]);
    let joins = bridges.iter().flatten().flatten().chain(neck_layers);
    for img in slimes.chain(joins) {
        for p in img.px.iter().filter(|p| p[3] > 0.05) {
            for under in [
                [205.0, 198.0, 176.0],
                [128.0, 118.0, 92.0],
                [60.0, 54.0, 44.0],
            ] {
                let c = mix(under, [p[0], p[1], p[2]], p[3]);
                samples.push(c.map(|v| v.clamp(0.0, 255.0) as u8));
            }
        }
    }
    // Score, tag and message strips are made at run time: sample black tape over the wall.
    let sample = dymo::strip("0123456789 YOUR MOVE", dymo::BLACK, 3, 0);
    for p in sample.px.iter().filter(|p| p[3] > 0.05) {
        for under in [[214.0, 204.0, 168.0], [150.0, 138.0, 100.0]] {
            let c = mix(under, [p[0], p[1], p[2]], p[3]);
            samples.push(c.map(|v| v.clamp(0.0, 255.0) as u8));
        }
    }
    let mut palette = [[0u8; 3]; 256];
    palette[..FIXED.len()].copy_from_slice(&FIXED);
    let cut = median_cut(samples, 256 - FIRST_FREE);
    palette[FIRST_FREE..FIRST_FREE + cut.len()].copy_from_slice(&cut);

    let mut q = Quantizer::new(palette);
    let mut background = Bitmap::new(W as u32, H as u32);
    for (i, c) in bg.iter().enumerate() {
        background.data[i] = q.get(*c, i % W, i / W);
    }
    let colonies = sprites.map(|side| {
        side.iter()
            .map(|variant| {
                variant
                    .iter()
                    .map(|img| {
                        let mut b = Bitmap::new(SPR as u32, SPR as u32);
                        for (i, p) in img.px.iter().enumerate() {
                            let (x, y) = (i % SPR, i / SPR);
                            if p[3] > bayer(x, y) {
                                b.data[i] = q.get([p[0], p[1], p[2]], x, y);
                            }
                        }
                        b
                    })
                    .collect()
            })
            .collect()
    });
    Scene {
        background,
        colonies,
        puddles,
        bridges,
        necks,
        palette,
        quant: q,
    }
}

fn bayer(x: usize, y: usize) -> f32 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (M[y % 4][x % 4] as f32 + 0.5) / 16.0
}

/// Nearest palette colour after a 4x4 Bayer nudge, cached on a 6-bit RGB grid.
pub struct Quantizer {
    pub pal: [[u8; 3]; 256],
    cache: Vec<u8>,
}

impl Quantizer {
    fn new(pal: [[u8; 3]; 256]) -> Self {
        Self {
            pal,
            cache: vec![0; 1 << 18],
        }
    }

    pub fn get(&mut self, c: [f32; 3], x: usize, y: usize) -> u8 {
        let t = (bayer(x, y) - 0.5) * 12.0;
        let k = c.map(|v| ((v + t).clamp(0.0, 255.0) as usize) >> 2);
        let key = k[0] << 12 | k[1] << 6 | k[2];
        if self.cache[key] == 0 {
            let c = k.map(|v| (v << 2) as f32 + 2.0);
            let mut best = (f32::MAX, 1);
            // Skip the transparent key and the UI colours.
            for (i, p) in self.pal.iter().enumerate().skip(FIRST_FREE) {
                let (dr, dg, db) = (c[0] - p[0] as f32, c[1] - p[1] as f32, c[2] - p[2] as f32);
                let d = 2.0 * dr * dr + 4.0 * dg * dg + 3.0 * db * db;
                if d < best.0 {
                    best = (d, i);
                }
            }
            self.cache[key] = best.1 as u8;
        }
        self.cache[key]
    }
}

fn median_cut(samples: Vec<[u8; 3]>, n: usize) -> Vec<[u8; 3]> {
    let mut boxes = vec![samples];
    while boxes.len() < n {
        // Split the box with the widest channel range (weighted by population).
        let Some((bi, ch, _)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .map(|(i, b)| {
                let (ch, range) = (0..3)
                    .map(|ch| {
                        let (lo, hi) = b
                            .iter()
                            .fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[ch]), hi.max(p[ch])));
                        (ch, (hi - lo) as f32)
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .unwrap();
                (i, ch, range * (b.len() as f32).sqrt())
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
        else {
            break;
        };
        let mut b = boxes.swap_remove(bi);
        b.sort_unstable_by_key(|p| p[ch]);
        let hi = b.split_off(b.len() / 2);
        boxes.push(b);
        boxes.push(hi);
    }
    boxes
        .iter()
        .map(|b| {
            let s = b.iter().fold([0u64; 3], |s, p| {
                [s[0] + p[0] as u64, s[1] + p[1] as u64, s[2] + p[2] as u64]
            });
            s.map(|v| (v / b.len().max(1) as u64) as u8)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_cell_roundtrip() {
        for q in -4..=4 {
            for r in -4..=4 {
                let (x, y) = cell_center(q, r);
                assert_eq!(pixel_to_cell(x + 3.0, y - 4.0), (q, r));
            }
        }
    }

    #[test]
    fn board_fits_left_of_the_labels() {
        let (x, _) = cell_center(4, 0);
        assert!(x + R < HUD_X as f32);
        let (_, y) = cell_center(0, 4);
        assert!(y + R < H as f32);
    }
}
