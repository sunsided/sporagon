//! All graphics are painted procedurally in true colour, then reduced to one 256-colour VGA
//! palette (median cut) with an ordered dither, the way 1990s artists prepared scanned art.

use game_toolkit_dosvideo::prelude::Bitmap;

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
pub const PEN: u8 = 5;
pub const BLOOD: u8 = 6;
pub const DIM: u8 = 7;
const FIXED: [[u8; 3]; 8] = [
    [0, 0, 0],
    [38, 30, 52],
    [120, 255, 90],
    [255, 214, 60],
    [252, 252, 244],
    [24, 40, 120],
    [170, 20, 20],
    [92, 84, 70],
];
const FIRST_FREE: usize = 16;

/// HUD layout on the right-hand wall: masking tape strips `(x, y, w, h)`.
pub const TAPE_TITLE: (i32, i32, i32, i32) = (500, 28, 132, 34);
pub const TAPE_PENI: (i32, i32, i32, i32) = (498, 96, 136, 78);
pub const TAPE_RHIZO: (i32, i32, i32, i32) = (498, 196, 136, 78);
pub const NOTE: (i32, i32, i32, i32) = (504, 304, 126, 136);

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

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xFF_FFFF) as f32 / 16_777_216.0
}

fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
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

fn fbm(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
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

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

fn scale(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
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
struct Rgba {
    px: Vec<[f32; 4]>,
}

impl Rgba {
    fn new() -> Self {
        Self {
            px: vec![[0.0; 4]; SPR * SPR],
        }
    }

    /// Paint `c` over the pixel with coverage `a`.
    fn over(&mut self, x: i32, y: i32, c: [f32; 3], a: f32) {
        if x < 0 || y < 0 || x >= SPR as i32 || y >= SPR as i32 || a <= 0.0 {
            return;
        }
        let p = &mut self.px[y as usize * SPR + x as usize];
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

/// Dome lighting for a colony: 0 at the rim, brighter towards the bulb (top right).
fn dome(dx: f32, dy: f32, nr: f32) -> f32 {
    let h = (1.0 - nr * nr).max(0.0).sqrt();
    let (nx, ny, nz) = (dx * 0.6, dy * 0.6, h + 0.2);
    let l = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-4);
    let d = (nx * 0.45 - ny * 0.6 + nz * 0.66) / l;
    0.72 + 0.4 * d.max(0.0)
}

/// Penicillium: velvety blue-green with concentric zones, radial furrows, a white advancing
/// margin of young hyphae, and amber guttation droplets on the older parts.
fn penicillium(seed: u32, g: f32) -> Rgba {
    let mut img = Rgba::new();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (seed as u64 * 7919));
    let c = SPR as f32 / 2.0;
    let rc = 4.0 + 19.0 * g;
    let furrows = 9.0 + (seed % 4) as f32 * 2.0;
    let rot = rng.range(0.0, std::f32::consts::TAU);
    for y in 0..SPR {
        for x in 0..SPR {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let r = (dx * dx + dy * dy).sqrt();
            let th = dy.atan2(dx) + rot;
            let wob = fbm(th.cos() * 2.0 + 5.0, th.sin() * 2.0 + 5.0, seed, 3);
            let edge = rc * (0.9 + 0.22 * wob);
            let nr = r / edge;
            let fine = fbm(x as f32 / 1.7, y as f32 / 1.7, seed + 11, 2);
            if nr < 1.0 {
                let core = [118.0, 132.0, 104.0];
                let velvet = [52.0, 104.0, 90.0];
                let velvet2 = [82.0, 132.0, 108.0];
                let rim = [158.0, 196.0, 160.0];
                let margin = [238.0, 240.0, 228.0];
                let zone = (nr * 7.0 + wob * 2.0).sin() * 0.5 + 0.5;
                let mut col = if nr < 0.3 {
                    mix(core, velvet, nr / 0.3)
                } else if nr < 0.72 {
                    mix(velvet, velvet2, zone)
                } else if nr < 0.88 {
                    mix(velvet2, rim, (nr - 0.72) / 0.16)
                } else {
                    mix(rim, margin, (nr - 0.88) / 0.12)
                };
                // Dusty bloom of conidia.
                let dust = smooth(0.55, 0.8, fbm(x as f32 / 4.0, y as f32 / 4.0, seed + 13, 3));
                col = mix(
                    col,
                    [150.0, 176.0, 158.0],
                    dust * 0.5 * (nr < 0.85) as u8 as f32,
                );
                // Young colonies are still mostly white mycelium.
                col = mix(col, margin, (1.0 - g * 1.4).max(0.0) * (1.0 - nr * 0.3));
                let furrow = ((th * furrows + wob * 4.0).sin() * 0.5 + 0.5).powi(6);
                let k = dome(dx / edge, dy / edge, nr)
                    * (1.0 - 0.25 * furrow * (nr * 2.0).min(1.0) * g)
                    * (0.84 + 0.32 * fine);
                col = scale(col, k);
                // Sporulating powder: bright and dark specks.
                let s = hash(x as i32, y as i32, seed + 3);
                if nr < 0.85 && s > 0.93 {
                    col = scale(col, 1.25);
                } else if nr < 0.85 && s < 0.05 {
                    col = scale(col, 0.7);
                }
                img.over(x as i32, y as i32, col, 1.0);
            } else if nr < 1.25 {
                // Fuzzy fringe of young hyphae, thinning out.
                let fuzz = fbm(x as f32 / 1.3, y as f32 / 1.3, seed + 7, 2);
                let a = (1.0 - (nr - 1.0) / 0.25).powf(1.5) * smooth(0.3, 0.7, fuzz);
                img.over(x as i32, y as i32, [222.0, 228.0, 216.0], a * 0.85);
            }
        }
    }
    // Guttation: little amber droplets of exudate.
    if g > 0.6 {
        for _ in 0..(seed % 3 + 2) {
            let a = rng.range(0.0, std::f32::consts::TAU);
            let d = rng.range(0.2, 0.6) * rc;
            let (px, py) = (c + a.cos() * d, c + a.sin() * d);
            let rr = rng.range(1.0, 2.2);
            img.disk(px + 0.6, py + 0.8, rr, [40.0, 50.0, 30.0], 0.5);
            img.disk(px, py, rr, [196.0, 150.0, 60.0], 1.0);
            img.over(
                (px + rr * 0.4) as i32,
                (py - rr * 0.5) as i32,
                [255.0, 250.0, 220.0],
                1.0,
            );
        }
    }
    img
}

/// Rhizopus, black bread mould: a tangle of white cottony hyphae dotted with sporangia that
/// start white and turn black as they ripen.
fn rhizopus(seed: u32, g: f32) -> Rgba {
    let mut img = Rgba::new();
    let mut rng = Rng(0xD1B5_4A32_D192_ED03 ^ (seed as u64 * 104_729));
    let c = SPR as f32 / 2.0;
    let rc = 4.0 + 20.0 * g;
    for y in 0..SPR {
        for x in 0..SPR {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let r = (dx * dx + dy * dy).sqrt();
            let th = dy.atan2(dx);
            let wob = fbm(th.cos() * 1.6 + 9.0, th.sin() * 1.6 + 9.0, seed, 3);
            let nr = r / (rc * (0.85 + 0.3 * wob));
            let fluff = fbm(x as f32 / 2.3, y as f32 / 2.3, seed + 5, 3);
            let dens = (1.0 - nr * nr).max(0.0) * (0.45 + 0.9 * fluff);
            if dens > 0.0 {
                let grey = lerp(150.0, 246.0, fluff.powf(0.8));
                let mut col = [grey, grey * 0.99, grey * 0.94];
                // Old centre: grey from spores that already fell.
                col = mix(col, [96.0, 94.0, 90.0], (1.0 - nr * 1.6).max(0.0) * g * 0.8);
                col = scale(col, dome(dx / rc, dy / rc, nr.min(1.0)));
                img.over(x as i32, y as i32, col, smooth(0.08, 0.4, dens));
            }
        }
    }
    // Aerial hyphae: wandering strands, past the colony edge too.
    for _ in 0..(20.0 + 70.0 * g) as u32 {
        let a0 = rng.range(0.0, std::f32::consts::TAU);
        let d0 = rng.range(0.0, 0.6) * rc;
        let (mut x, mut y) = (c + a0.cos() * d0, c + a0.sin() * d0);
        let mut a = a0 + rng.range(-0.8, 0.8);
        let len = rng.range(0.5, 1.0) * rc * 1.1;
        let mut t = 0.0;
        while t < len {
            a += rng.range(-0.35, 0.35);
            x += a.cos() * 0.7;
            y += a.sin() * 0.7;
            t += 0.7;
            let v = rng.range(205.0, 245.0);
            let out = ((x - c).powi(2) + (y - c).powi(2)).sqrt() / rc;
            img.over(
                x as i32,
                y as i32,
                [v, v, v * 0.96],
                if out > 0.95 { 0.35 } else { 0.6 },
            );
        }
    }
    // Sporangia on their stalks.
    let n = (44.0 * g * g) as u32;
    for _ in 0..n {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let d = rng.f().sqrt() * 0.9 * rc;
        let (px, py) = (c + a.cos() * d, c + a.sin() * d);
        let rr = rng.range(0.9, 2.2);
        let ripe = rng.f() < g * 0.95;
        // The stalk seen at a slant, and its shadow.
        for k in 0..3 {
            img.over(
                (px - 0.5 - k as f32 * 0.4) as i32,
                (py + 1.0 + k as f32) as i32,
                [176.0, 172.0, 160.0],
                0.7,
            );
        }
        img.disk(px + 1.2, py + 1.5, rr, [50.0, 48.0, 44.0], 0.35);
        let body = if ripe {
            [22.0, 22.0, 26.0]
        } else {
            [214.0, 210.0, 196.0]
        };
        img.disk(px, py, rr, body, 1.0);
        let hi = if ripe {
            [120.0, 124.0, 130.0]
        } else {
            [255.0, 255.0, 250.0]
        };
        img.over((px + rr * 0.35) as i32, (py - rr * 0.45) as i32, hi, 0.9);
    }
    img
}

// ------------------------------------------------------------------------------------------
// The fridge

struct Drop {
    x: f32,
    y: f32,
    r: f32,
}

fn droplets(rng: &mut Rng, n: usize, x0: f32, y0: f32, x1: f32, y1: f32, rmax: f32) -> Vec<Drop> {
    (0..n)
        .map(|_| Drop {
            x: rng.range(x0, x1),
            y: rng.range(y0, y1),
            r: rng.range(0.8, 1.0) * rng.range(0.6, 1.0) * rmax,
        })
        .collect()
}

/// A condensation drop acts as a tiny lens: darker rim, bright specular toward the bulb.
fn apply_drop(col: [f32; 3], d: &Drop, x: f32, y: f32) -> Option<[f32; 3]> {
    let (dx, dy) = ((x - d.x) / d.r, (y - d.y) / d.r);
    let rr = dx * dx + dy * dy;
    if rr > 1.0 {
        return None;
    }
    let spec = ((dx - 0.35).powi(2) + (dy + 0.4).powi(2)) < 0.07;
    if spec {
        return Some([255.0, 255.0, 245.0]);
    }
    let rim = smooth(0.55, 1.0, rr);
    // Light refracted through the lower half.
    let lens = if dy > 0.2 { 1.12 } else { 0.92 };
    Some(scale(col, lens * (1.0 - 0.35 * rim)))
}

/// Height of the egg tray (z up, in pixels) or `None` outside it.
fn tray_height(x: f32, y: f32, centers: &[(f32, f32, bool)]) -> Option<(f32, usize)> {
    let mut best = (f32::MAX, 0);
    for (i, &(cx, cy, _)) in centers.iter().enumerate() {
        let d = (x - cx).powi(2) + (y - cy).powi(2);
        if d < best.0 {
            best = (d, i);
        }
    }
    let d = best.0.sqrt();
    if d > R * 1.06 {
        return None;
    }
    let (cx, cy, _) = centers[best.1];
    let h = hex_norm(x - cx, y - cy);
    // A round cup pressed into a flat deck that rises to a ridge on the hex edges.
    let deck = -2.5 * (1.0 - smooth(0.78, 1.0, h));
    let rr = d / (R * 0.8);
    let cup = if rr < 1.0 {
        -12.0 * (1.0 - rr * rr).powf(0.8)
    } else {
        0.0
    };
    let edge = smooth(R * 0.95, R * 1.06, d);
    Some((deck + cup - edge * 7.0, best.1))
}

pub struct Scene {
    pub background: Bitmap,
    /// `[side][variant][stage]`.
    pub colonies: [Vec<Vec<Bitmap>>; 2],
    pub palette: [[u8; 3]; 256],
}

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
    let bg = paint_fridge(centers);

    let mut samples: Vec<[u8; 3]> = bg
        .iter()
        .step_by(3)
        .map(|c| c.map(|v| v.clamp(0.0, 255.0) as u8))
        .collect();
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
    let mut palette = [[0u8; 3]; 256];
    palette[..FIXED.len()].copy_from_slice(&FIXED);
    let cut = median_cut(samples, 256 - FIRST_FREE);
    palette[FIRST_FREE..FIRST_FREE + cut.len()].copy_from_slice(&cut);

    let mut q = Quantizer::new(&palette);
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
        palette,
    }
}

fn bayer(x: usize, y: usize) -> f32 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (M[y % 4][x % 4] as f32 + 0.5) / 16.0
}

/// Nearest palette colour after a 4x4 Bayer nudge, cached on a 6-bit RGB grid.
struct Quantizer<'a> {
    pal: &'a [[u8; 3]; 256],
    cache: Vec<u8>,
}

impl<'a> Quantizer<'a> {
    fn new(pal: &'a [[u8; 3]; 256]) -> Self {
        Self {
            pal,
            cache: vec![0; 1 << 18],
        }
    }

    fn get(&mut self, c: [f32; 3], x: usize, y: usize) -> u8 {
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

fn paint_fridge(centers: &[(f32, f32, bool)]) -> Vec<[f32; 3]> {
    let mut rng = Rng(0x5EED_F00D_CAFE_1234);
    let bulb = (586.0f32, 18.0f32);
    // Brown drips running down the back wall.
    let drips: Vec<(f32, f32, f32, f32)> = (0..11)
        .map(|_| {
            (
                rng.range(30.0, 620.0),
                rng.range(0.0, 200.0),
                rng.range(60.0, 300.0),
                rng.range(1.5, 4.0),
            )
        })
        .collect();
    let wall_drops = droplets(&mut rng, 160, 0.0, 0.0, W as f32, H as f32, 3.2);
    let tray_drops = droplets(
        &mut rng,
        140,
        BX - 240.0,
        BY - 215.0,
        BX + 240.0,
        BY + 215.0,
        2.6,
    );
    let tapes = [TAPE_TITLE, TAPE_PENI, TAPE_RHIZO];
    // Cracks spreading from the knocked-out cups.
    let mut cracks = vec![false; W * H];
    for &(cx, cy, hole) in centers {
        if !hole {
            continue;
        }
        for _ in 0..4 {
            let mut a = rng.range(0.0, std::f32::consts::TAU);
            let (mut x, mut y) = (cx + a.cos() * R * 0.7, cy + a.sin() * R * 0.7);
            for _ in 0..rng.range(10.0, 30.0) as u32 {
                a += rng.range(-0.5, 0.5);
                x += a.cos();
                y += a.sin();
                if (0.0..W as f32).contains(&x) && (0.0..H as f32).contains(&y) {
                    cracks[y as usize * W + x as usize] = true;
                }
            }
        }
    }

    let mut out = vec![[0.0; 3]; W * H];
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32, y as f32);
            out[y * W + x] = fridge_pixel(
                fx,
                fy,
                bulb,
                &drips,
                &wall_drops,
                &tray_drops,
                &tapes,
                centers,
                &cracks,
            );
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn fridge_pixel(
    fx: f32,
    fy: f32,
    bulb: (f32, f32),
    drips: &[(f32, f32, f32, f32)],
    wall_drops: &[Drop],
    tray_drops: &[Drop],
    tapes: &[(i32, i32, i32, i32)],
    centers: &[(f32, f32, bool)],
    cracks: &[bool],
) -> [f32; 3] {
    let dist_bulb = ((fx - bulb.0).powi(2) + (fy - bulb.1).powi(2)).sqrt();
    let light = 0.62 + 0.7 * (-dist_bulb / 380.0).exp();
    let vignette = 1.0 - 0.35 * (((fx - 320.0) / 340.0).powi(2) + ((fy - 240.0) / 280.0).powi(2));

    // Tray first: it sits in front of everything but the notes.
    if let Some((z, ci)) = tray_height(fx, fy, centers) {
        let (cx, cy, hole) = centers[ci];
        let h = hex_norm(fx - cx, fy - cy);
        if hole {
            let jag = 0.78 + 0.16 * fbm(fx / 5.0, fy / 5.0, 77, 2);
            if h < jag {
                // Broken out: the dark gap under the tray, a glint of wet shelf.
                let depth = 1.0 - h / jag;
                let wet = fbm(fx / 9.0, fy / 9.0, 78, 3);
                return scale([30.0, 26.0, 20.0], (0.35 + 0.5 * wet) * (1.0 - 0.6 * depth));
            }
            if h < jag + 0.06 {
                return scale([246.0, 242.0, 226.0], light * 1.05);
            }
        }
        let e = 0.8;
        let zx = tray_height(fx + e, fy, centers).map_or(z - 4.0, |t| t.0) - z;
        let zy = tray_height(fx, fy + e, centers).map_or(z - 4.0, |t| t.0) - z;
        let (nx, ny, nz) = (-zx / e, -zy / e, 1.0);
        let l = (nx * nx + ny * ny + nz * nz).sqrt();
        let (nx, ny, nz) = (nx / l, ny / l, nz / l);
        let diffuse = (nx * 0.45 - ny * 0.6 + nz * 0.66).max(0.0);
        // Blinn specular, viewer straight above.
        let (hx, hy, hz) = (0.45, -0.6, 1.66);
        let hl = hx * hx + hy * hy + hz * hz;
        let spec = ((nx * hx + ny * hy + nz * hz) / hl.sqrt())
            .max(0.0)
            .powi(40);
        let mut col = [228.0, 222.0, 200.0];
        // Grime settles in the bottom of the cups.
        let settle = ((1.0 - h).max(0.0)).powi(2) * fbm(fx / 7.0, fy / 7.0, 21, 3);
        col = mix(col, [150.0, 124.0, 74.0], settle * 0.7);
        col = mix(
            col,
            [190.0, 176.0, 130.0],
            smooth(0.5, 0.8, fbm(fx / 40.0, fy / 40.0, 22, 4)) * 0.6,
        );
        let shade = 0.5 + 0.62 * diffuse;
        col = scale(col, shade * light.min(1.15) * vignette.max(0.6));
        col = mix(col, [255.0, 252.0, 236.0], spec * 0.9);
        if cracks[fy as usize * W + fx as usize] {
            col = scale(col, 0.45);
        }
        if let Some(c) = tray_drops.iter().find_map(|d| apply_drop(col, d, fx, fy)) {
            col = c;
        }
        return col;
    }

    // Notes on the wall.
    for (i, &(tx, ty, tw, th)) in tapes.iter().enumerate() {
        let ragged = hash(fy as i32 / 2, i as i32, 5) * 4.0;
        if fx >= tx as f32 + ragged
            && fx < (tx + tw) as f32 - ragged * 0.7
            && fy >= ty as f32
            && fy < (ty + th) as f32
        {
            let fiber = fbm(fx / 1.5, fy / 6.0, 41, 2);
            let col = mix(
                [232.0, 214.0, 162.0],
                [200.0, 178.0, 120.0],
                fiber * 0.6 + fbm(fx / 20.0, fy / 20.0, 42, 2) * 0.4,
            );
            return scale(col, light.min(1.1) * 0.95);
        }
    }
    let (nx, ny, nw, nh) = NOTE;
    if fx >= nx as f32 && fx < (nx + nw) as f32 && fy >= ny as f32 && fy < (ny + nh) as f32 {
        // A sticky note gone soft and wavy from the damp.
        let wave = (fy / 7.0).sin() * 0.04 + fbm(fx / 12.0, fy / 12.0, 51, 3) * 0.12;
        let curl = smooth(0.75, 1.0, (fy - ny as f32) / nh as f32) * 0.25;
        let mut col = scale([238.0, 226.0, 128.0], light.min(1.1) * (0.9 + wave - curl));
        if fy < (ny + 22) as f32 {
            col = scale(col, 0.9);
        }
        // A water stain ring.
        let ring = (((fx - 590.0).powi(2) + (fy - 420.0).powi(2)).sqrt() - 26.0).abs();
        if ring < 2.0 {
            col = mix(col, [170.0, 140.0, 60.0], 0.5);
        }
        return col;
    }

    // Back wall: yellowed plastic, a darker band where the shelf rails run.
    let n = fbm(fx / 90.0, fy / 90.0, 1, 4);
    let fine = fbm(fx / 3.0, fy / 3.0, 2, 2);
    let mut col = mix([226.0, 220.0, 190.0], [198.0, 184.0, 136.0], n);
    col = scale(col, 0.92 + 0.12 * fine);
    for rail in [118.0f32, 300.0, 452.0] {
        let d = fy - rail;
        if (0.0..3.0).contains(&d) {
            col = scale(col, 1.18);
        } else if (3.0..9.0).contains(&d) {
            col = scale(col, 0.62 + 0.05 * (d - 3.0));
        }
    }
    // Side walls in perspective, ribbed.
    let side = if fx < 34.0 - fy * 0.02 {
        Some(fx)
    } else if fx > 606.0 + fy * 0.02 {
        Some(W as f32 - fx)
    } else {
        None
    };
    if let Some(s) = side {
        col = scale(col, 0.62 + s * 0.006);
        if ((fy + s * 1.5) % 26.0) < 3.0 {
            col = scale(col, 0.75);
        }
    }
    // Brown stains and drips.
    let stain = smooth(0.58, 0.8, fbm(fx / 55.0, fy / 35.0, 7, 5));
    col = mix(col, [150.0, 112.0, 60.0], stain * 0.55);
    for &(x0, y0, len, w) in drips {
        let t = (fy - y0) / len;
        if (0.0..1.0).contains(&t) {
            let cx = x0 + (fy / 23.0).sin() * 1.5 + fbm(fy / 30.0, x0, 9, 2) * 4.0;
            let width = w * (1.0 - t * 0.7) + if t > 0.93 { 1.5 } else { 0.0 };
            let d = (fx - cx).abs();
            if d < width {
                col = mix(col, [128.0, 88.0, 40.0], 0.55 + 0.3 * (d / width));
            }
        }
    }
    // Black mildew (Cladosporium) colonising the damp corners and the rails.
    let corner = [(0.0, 480.0), (640.0, 480.0), (0.0, 0.0)]
        .iter()
        .map(|&(cx, cy): &(f32, f32)| {
            (-((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() / 110.0).exp()
        })
        .fold(0.0f32, f32::max);
    let railness = [118.0f32, 300.0, 452.0]
        .iter()
        .map(|r| (-(fy - r - 6.0).abs() / 6.0).exp())
        .fold(0.0, f32::max)
        * 0.5;
    let mildew = (corner + railness) * fbm(fx / 14.0, fy / 14.0, 31, 3);
    let spot = vnoise(fx / 1.8, fy / 1.8, 32);
    if spot > 1.0 - mildew * 0.9 {
        col = mix(col, [26.0, 30.0, 24.0], 0.75 + 0.25 * spot);
    } else if spot > 1.0 - mildew * 1.3 {
        col = mix(col, [70.0, 72.0, 52.0], 0.4);
    }
    col = scale(col, light * vignette.max(0.35));
    // Drop shadow of the tray and the notes.
    let shadowed = tray_height(fx - 9.0, fy - 12.0, centers).is_some()
        || tapes.iter().chain([&NOTE]).any(|&(x, y, w, h)| {
            fx - 4.0 >= x as f32
                && fx - 4.0 < (x + w) as f32
                && fy - 5.0 >= y as f32
                && fy - 5.0 < (y + h) as f32
        });
    if shadowed {
        col = scale(col, 0.58);
    }
    // The thermostat knob, stuck on 7 since 1987.
    let (kx, ky) = (fx - 52.0, fy - 52.0);
    let kd = (kx * kx + ky * ky).sqrt();
    if kd < 21.0 {
        let a = ky.atan2(kx);
        let grip = if kd > 15.0 && (a * 18.0).sin() > 0.3 {
            0.8
        } else {
            1.0
        };
        let dome = 1.05 - -(0.4 * ((kx - 6.0) * 0.6 / 21.0 + (ky + 6.0) * 0.8 / 21.0).max(-1.0));
        col = scale([214.0, 210.0, 196.0], dome.clamp(0.5, 1.3) * grip * light);
        // The pointer, at 7.
        let (px, py) = ((-0.8f32).cos(), (-0.8f32).sin());
        let along = kx * px + ky * py;
        if (kx * py - ky * px).abs() < 1.6 && (2.0..14.0).contains(&along) {
            col = [150.0, 30.0, 24.0];
        }
        if kd > 20.0 {
            col = scale(col, 0.5);
        }
        col = mix(
            col,
            [120.0, 96.0, 50.0],
            fbm(fx / 4.0, fy / 4.0, 71, 2) * 0.35,
        );
    } else if kd < 24.0 {
        col = scale(col, 0.6);
    }
    // The bulb in its cage, rusted at the mount.
    let db = dist_bulb;
    let glow = (-db / 60.0).exp() * 0.5;
    col = mix(col, [255.0, 240.0, 190.0], glow);
    if db < 15.0 {
        let core = 1.0 - db / 15.0;
        col = mix([255.0, 214.0, 120.0], [255.0, 255.0, 236.0], core * 1.4);
        if ((fx - bulb.0) as i32).rem_euclid(7) == 0 && db > 4.0 {
            col = [70.0, 62.0, 50.0];
        }
    } else if db < 22.0 {
        col = mix(
            [120.0, 70.0, 36.0],
            [62.0, 40.0, 26.0],
            fbm(fx / 2.0, fy / 2.0, 61, 2),
        );
    }
    if let Some(c) = wall_drops.iter().find_map(|d| apply_drop(col, d, fx, fy)) {
        col = c;
    }
    col
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
    fn board_fits_left_of_the_notes() {
        let (x, _) = cell_center(4, 0);
        assert!(x + R < TAPE_PENI.0 as f32);
        let (_, y) = cell_center(0, 4);
        assert!(y + R < H as f32);
    }
}
