//! All graphics are painted procedurally in true colour, then reduced to one 256-colour VGA
//! palette (median cut) with an ordered dither, the way 1990s artists prepared scanned art.
//!
//! Conventions shared by the painting modules: coordinates are pixels with y pointing down,
//! colours are RGB 0..255 as `f32` (values may stray outside and are clamped on quantising),
//! coverage (alpha) is 0..1, and the only light is the fridge bulb at the top right.

use game_toolkit_dosvideo::prelude::Bitmap;

pub mod dymo;
mod fridge;
pub mod slime;
mod spores;

use fridge::paint_fridge;
use spores::{penicillium, penicillium_neck, rhizopus, rhizopus_neck};

/// Screen width in pixels.
pub const W: usize = 640;
/// Screen height in pixels.
pub const H: usize = 480;
/// Hex cell circumradius (pointy-top cells).
pub const R: f32 = 30.0;
/// Screen x of the board centre, cell `(0, 0)`.
pub const BX: f32 = 250.0;
/// Screen y of the board centre, cell `(0, 0)`.
pub const BY: f32 = 240.0;
/// √3, for hex geometry.
const SQ3: f32 = 1.732_050_8;
/// Colony sprites are `SPR x SPR`, centred on the cell.
pub const SPR: usize = 64;
/// Growth stages per colony sprite set; stage `s` has growth `(s + 1) / STAGES`.
pub const STAGES: usize = 10;
/// Differently seeded variants per side and stage, so neighbouring colonies differ.
pub const VARIANTS: usize = 4;

/// Fixed palette entries; 0 doubles as the sprites' transparent key.
pub const TRANSPARENT: u8 = 0;
/// Palette index of the dark UI ink.
pub const INK: u8 = 1;
/// Palette index of the green highlight for the near move targets.
pub const GLOW_NEAR: u8 = 2;
/// Palette index of the amber highlight for the far move targets.
pub const GLOW_FAR: u8 = 3;
/// Palette index of the UI white.
pub const WHITE: u8 = 4;
/// RGB of the fixed entries, by index.
const FIXED: [[u8; 3]; 5] = [
    [0, 0, 0],
    [38, 30, 52],
    [120, 255, 90],
    [255, 214, 60],
    [252, 252, 244],
];
/// First palette index the median cut may fill (and the quantiser may pick).
const FIRST_FREE: usize = FIXED.len();

/// The fridge bulb, top right.
const BULB: (f32, f32) = (586.0, 18.0);

/// HUD column on the right-hand wall: left edge, width, and the centre every strip hangs on.
pub const HUD_X: i32 = 496;
/// Width of the HUD column in pixels.
pub const HUD_W: usize = 140;
/// Screen x every HUD strip is centred on.
pub const HUD_CX: i32 = HUD_X + HUD_W as i32 / 2;
/// Strip centres (y) for the title, each side's name, the first message line and the keys.
pub const TITLE_Y: i32 = 54;
/// Strip centre (y) of each side's name, `[Penicillium, Rhizopus]`.
pub const SIDE_Y: [i32; 2] = [102, 216];
/// Strip centre (y) of the first message line.
pub const MESSAGE_Y: i32 = 336;
/// Vertical distance between message lines.
pub const MESSAGE_STEP: i32 = 27;
/// Strip centre (y) of the key help.
pub const KEYS_Y: i32 = 442;
/// Title on the top strip.
pub const GAME_TITLE: &str = "SPORAGON";
/// Side names, indexed by side.
pub const SIDE_NAMES: [&str; 2] = ["PENICILLIUM", "RHIZOPUS"];
/// Key help on the bottom strip.
pub const KEYS: &str = "N NEW D DEMO";

/// Screen pixel centre of the axial cell `(q, r)` (pointy-top hexes, `r` grows downward).
pub fn cell_center(q: i32, r: i32) -> (f32, f32) {
    (
        BX + R * SQ3 * (q as f32 + r as f32 / 2.0),
        BY + R * 1.5 * r as f32,
    )
}

/// The axial cell under a pixel (may lie off the board).
///
/// 1. Convert the pixel to fractional axial coordinates (the inverse of [`cell_center`]).
/// 2. Round in cube coordinates `(q, r, s)` with `q + r + s = 0`: round each, then recompute
///    the one with the largest rounding error from the other two, so the result is a valid
///    cell.
pub fn pixel_to_cell(x: f32, y: f32) -> (i32, i32) {
    let (px, py) = ((x - BX) / R, (y - BY) / R);
    let q = SQ3 / 3.0 * px - py / 3.0;
    let r = 2.0 / 3.0 * py;

    // Cube rounding.
    let s = -q - r;
    let (mut round_q, mut round_r, round_s) = (q.round(), r.round(), s.round());
    let (err_q, err_r, err_s) = (
        (round_q - q).abs(),
        (round_r - r).abs(),
        (round_s - s).abs(),
    );
    if err_q > err_r && err_q > err_s {
        round_q = -round_r - round_s;
    } else if err_r > err_s {
        round_r = -round_q - round_s;
    }
    (round_q as i32, round_r as i32)
}

/// 0 at the cell centre, 1 on its edge (hexagonal distance).
///
/// `(dx, dy)` is the pixel offset from the centre. The distance is the largest projection onto
/// the three edge normals of a pointy-top hex (0°, ±60°), divided by the inradius `R √3 / 2`.
fn hex_norm(dx: f32, dy: f32) -> f32 {
    let along_x = dx.abs();
    let along_60 = (dx * 0.5 + dy * SQ3 / 2.0).abs();
    let along_minus_60 = (dx * 0.5 - dy * SQ3 / 2.0).abs();
    along_x.max(along_60).max(along_minus_60) / (R * SQ3 / 2.0)
}

// ------------------------------------------------------------------------------------------
// Noise

/// Deterministic hash of an integer lattice point to 0..1 (24 bits of precision).
///
/// Multiplies each input by a large odd constant, xors them and runs a short avalanche
/// (xorshift, multiply, xorshift). Same inputs always give the same value.
pub(crate) fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut state = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    state = (state ^ (state >> 13)).wrapping_mul(1_274_126_177);
    state ^= state >> 16;

    // Keep the low 24 bits, the most an f32 mantissa holds exactly; 2^24 = 16 777 216.
    (state & 0xFF_FFFF) as f32 / 16_777_216.0
}

/// Value noise in 0..1: [`hash`] at the four surrounding lattice points, blended with a
/// smoothstep so the result is continuous with a continuous slope. One lattice cell is one
/// unit of `x` and `y`.
pub(crate) fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (frac_x, frac_y) = (x - x0, y - y0);
    // Smoothstep 3t² - 2t³ of the fractions.
    let (ease_x, ease_y) = (
        frac_x * frac_x * (3.0 - 2.0 * frac_x),
        frac_y * frac_y * (3.0 - 2.0 * frac_y),
    );

    let (ix, iy) = (x0 as i32, y0 as i32);
    let top_left = hash(ix, iy, seed);
    let top_right = hash(ix + 1, iy, seed);
    let bottom_left = hash(ix, iy + 1, seed);
    let bottom_right = hash(ix + 1, iy + 1, seed);

    let top = lerp(top_left, top_right, ease_x);
    let bottom = lerp(bottom_left, bottom_right, ease_x);
    lerp(top, bottom, ease_y)
}

/// Fractal (fBm) value noise in 0..1, summing `octaves` layers of [`vnoise`].
///
/// 1. Octave `o` is sampled at frequency `2.03^o` with seed `seed + 101 o`, so layers are
///    decorrelated. The 2.03 (rather than 2) keeps the lattices of successive octaves from
///    lining up into visible grid artefacts.
/// 2. Each octave has half the amplitude of the one before.
/// 3. The sum is divided by the total amplitude, so the result stays in 0..1.
pub(crate) fn fbm(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amplitude, mut total_amplitude, mut frequency) = (0.0, 1.0, 0.0, 1.0);
    for octave in 0..octaves {
        sum += amplitude * vnoise(x * frequency, y * frequency, seed + octave * 101);
        total_amplitude += amplitude;
        amplitude *= 0.5;
        frequency *= 2.03;
    }
    sum / total_amplitude
}

/// Linear interpolation from `a` (at `t = 0`) to `b` (at `t = 1`); `t` is not clamped.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Per-channel blend of two RGB colours, `t` clamped to 0..1 (0 gives `a`, 1 gives `b`).
pub(crate) fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

/// Multiply each channel of `colour` by `factor` (brighten above 1, darken below; no clamp).
pub(crate) fn scale(colour: [f32; 3], factor: f32) -> [f32; 3] {
    [colour[0] * factor, colour[1] * factor, colour[2] * factor]
}

/// Direction to the bulb: top right, in front of the tray (screen space, y down).
/// Roughly unit length.
const LIGHT: [f32; 3] = [0.45, -0.6, 0.66];

/// Lambert term for the unit normal `normal` (not clamped): -1..1, 1 facing the bulb.
pub(crate) fn facing(normal: [f32; 3]) -> f32 {
    normal[0] * LIGHT[0] + normal[1] * LIGHT[1] + normal[2] * LIGHT[2]
}

/// Blinn term for the unit normal `normal` with the viewer straight above (not clamped).
///
/// The half vector is `LIGHT + (0, 0, 1)` normalised, so `normal · half` expands to
/// `(facing(normal) + normal.z) / |LIGHT + (0, 0, 1)|`. Raise it to a power for the highlight.
pub(crate) fn glint(normal: [f32; 3]) -> f32 {
    // |LIGHT + (0, 0, 1)|
    const HALF_LEN: f32 = 1.8216;
    (facing(normal) + normal[2]) / HALF_LEN
}

/// Smoothstep: 0 for `x <= e0`, 1 for `x >= e1`, and a smooth `3t² - 2t³` ramp in between.
/// Assumes `e0 != e1`.
pub(crate) fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Small deterministic RNG for scattering things (xorshift64). The state must not be 0.
pub struct Rng(pub u64);

impl Rng {
    /// Next 32 random bits (the high half of the 64-bit state).
    pub fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }

    /// Uniform float in 0..1 (24 bits of precision).
    pub fn f(&mut self) -> f32 {
        (self.next() >> 8) as f32 / 16_777_216.0
    }

    /// Uniform float in `a..b`.
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

// ------------------------------------------------------------------------------------------
// Colonies

/// A square true-colour sprite with coverage.
///
/// Pixels are row-major RGBA: colour in 0..255, alpha 0..1. The colour is stored
/// non-premultiplied; a pixel with alpha 0 is empty and its colour is meaningless.
pub struct Rgba {
    /// Width and height in pixels.
    pub w: usize,
    /// `w * w` pixels, row-major, `[r, g, b, alpha]`.
    pub px: Vec<[f32; 4]>,
}

impl Rgba {
    /// A fully transparent `w x w` sprite.
    pub fn new(w: usize) -> Self {
        Self {
            w,
            px: vec![[0.0; 4]; w * w],
        }
    }

    /// Paint `colour` over the pixel with coverage `alpha`.
    ///
    /// Standard "over" compositing on non-premultiplied colour:
    /// 1. Ignore pixels outside the sprite and `alpha <= 0`; clamp `alpha` to at most 1.
    /// 2. New alpha: `alpha + old_alpha (1 - alpha)`.
    /// 3. New colour: the alpha-weighted sum of new and old colour, divided by the new alpha
    ///    (guarded against division by zero).
    pub fn over(&mut self, x: i32, y: i32, colour: [f32; 3], alpha: f32) {
        let w = self.w as i32;
        if x < 0 || y < 0 || x >= w || y >= w || alpha <= 0.0 {
            return;
        }

        let pixel = &mut self.px[(y * w + x) as usize];
        let alpha = alpha.min(1.0);
        let new_alpha = alpha + pixel[3] * (1.0 - alpha);
        for i in 0..3 {
            pixel[i] =
                (colour[i] * alpha + pixel[i] * pixel[3] * (1.0 - alpha)) / new_alpha.max(1e-6);
        }
        pixel[3] = new_alpha;
    }

    /// Paint an anti-aliased filled disc centred at `(cx, cy)` (sprite pixels) with radius
    /// `radius`, at peak coverage `alpha`. The edge is one pixel wide: coverage ramps from
    /// full at `radius - 0.5` to zero at `radius + 0.5`, measured to the pixel centre.
    fn disk(&mut self, cx: f32, cy: f32, radius: f32, colour: [f32; 3], alpha: f32) {
        let (x0, x1) = ((cx - radius - 1.0) as i32, (cx + radius + 1.0) as i32);
        let (y0, y1) = ((cy - radius - 1.0) as i32, (cy + radius + 1.0) as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dist = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let edge = (radius + 0.5 - dist).clamp(0.0, 1.0);
                self.over(x, y, colour, alpha * edge);
            }
        }
    }
}

/// Everything painted once at start-up, already quantised to the shared palette where it is
/// opaque, and kept in true colour where it is translucent and must be blended at draw time.
pub struct Scene {
    /// The fridge, tray and fixed HUD labels, `W x H`, palette indices.
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
    /// The 256-colour VGA palette, RGB 0..255.
    pub palette: [[u8; 3]; 256],
    /// Quantiser for `palette`, for everything blended at run time.
    pub quant: Quantizer,
}

/// Side length in pixels of the bridge and neck sprites.
pub const BRIDGE: usize = 80;
/// Differently seeded variants per bridge direction.
pub const BRIDGE_VARIANTS: usize = 3;
/// Axial offsets of the neighbours bridges are drawn toward (the other three are the same
/// bridges seen from the far end).
pub const BRIDGE_DIRS: [(i32, i32); 3] = [(1, 0), (1, -1), (0, -1)];

/// Clean-to-grimy tray plastic that slime is sampled over for the palette, RGB.
const SLIME_UNDERLAYS: [[f32; 3]; 3] = [
    [205.0, 198.0, 176.0],
    [128.0, 118.0, 92.0],
    [60.0, 54.0, 44.0],
];

/// Light and dark wall that run-time tape strips are sampled over for the palette, RGB.
const TAPE_UNDERLAYS: [[f32; 3]; 2] = [[214.0, 204.0, 168.0], [150.0, 138.0, 100.0]];

/// Paint everything. `centers` are the board cells, `true` for a knocked-out hole.
///
/// 1. Paint the colony sprites for both sides, every variant and growth stage, in true colour.
/// 2. Paint the fridge background and bake the fixed Dymo labels into it.
/// 3. Render the slime puddles, bridges and mould necks as translucent RGBA sprites.
/// 4. Gather colour samples, weighted towards what draws the eye: every third background pixel,
///    the bulb's cage (x6), the mature colonies (x3), the slime composited over three shades of
///    plastic, and black tape over the wall for the strips made at run time.
/// 5. Build the palette: the fixed UI entries, then median cut over the samples.
/// 6. Quantise the background and colony sprites with the ordered dither; colony pixels whose
///    alpha does not beat the Bayer threshold stay transparent (index 0).
///
/// The order of every step is significant: it fixes the palette and so every pixel on screen.
pub fn paint(centers: &[(f32, f32, bool)]) -> Scene {
    let sprites: [Vec<Vec<Rgba>>; 2] = [0u32, 1].map(|side| {
        (0..VARIANTS as u32)
            .map(|variant| {
                (0..STAGES)
                    .map(|stage| {
                        let growth = (stage + 1) as f32 / STAGES as f32;
                        if side == 0 {
                            penicillium(variant * 31 + 1, growth)
                        } else {
                            rhizopus(variant * 37 + 2, growth)
                        }
                    })
                    .collect()
            })
            .collect()
    });

    let mut background_rgb = paint_fridge(centers);

    // The labels that never change are stuck on the wall for good.
    let tapes = [dymo::GREEN, dymo::GREY];
    for (text, tape, y) in [
        (GAME_TITLE, dymo::RED, TITLE_Y),
        (KEYS, dymo::BLACK, KEYS_Y),
    ]
    .into_iter()
    .chain((0..2).map(|s| (SIDE_NAMES[s], tapes[s], SIDE_Y[s])))
    {
        let strip = dymo::strip(text, tape, 2, y as u32);
        dymo::bake(&mut background_rgb, W, &strip, HUD_CX, y);
    }

    let puddles: [Vec<Vec<Rgba>>; 2] = [0usize, 1].map(|side| {
        (0..VARIANTS as u32)
            .map(|variant| {
                (0..STAGES)
                    .map(|stage| {
                        let growth = (stage + 1) as f32 / STAGES as f32;
                        let seed = variant * 13 + side as u32;
                        slime::to_rgba(&slime::puddle(seed, growth, SPR), side, SPR)
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
                .map(|variant| {
                    let seed = variant * 7 + side as u32;
                    slime::to_rgba(
                        &slime::bridge((x1 - x0, y1 - y0), seed, BRIDGE),
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
            let offset = (x1 - x0, y1 - y0);
            (0..BRIDGE_VARIANTS as u32)
                .map(|variant| {
                    let seed = variant * 19 + 3;
                    if side == 0 {
                        penicillium_neck(offset, seed, BRIDGE)
                    } else {
                        rhizopus_neck(offset, seed, BRIDGE)
                    }
                })
                .collect()
        })
    });

    // Palette samples, RGB 0..255. Repeating a sample gives it more weight in the median cut.
    let to_u8 = |colour: [f32; 3]| colour.map(|v| v.clamp(0.0, 255.0) as u8);
    let mut samples: Vec<[u8; 3]> = background_rgb
        .iter()
        .step_by(3)
        .map(|c| to_u8(*c))
        .collect();

    // The bulb's rusty cage is small but catches the eye: weight it so it keeps its browns.
    for y in 0..40 {
        for x in (BULB.0 as usize - 24)..(BULB.0 as usize + 24) {
            samples.extend([to_u8(background_rgb[y * W + x]); 6]);
        }
    }

    for side in &sprites {
        for variant in side {
            for img in variant.iter().skip(STAGES - 3) {
                // Weight the mould: it is what the player looks at.
                for pixel in img.px.iter().filter(|p| p[3] > 0.5) {
                    let colour = to_u8([pixel[0], pixel[1], pixel[2]]);
                    samples.extend([colour; 3]);
                }
            }
        }
    }

    // Slime is translucent: sample it over clean and grimy plastic, as it will be seen.
    let slimes = puddles
        .iter()
        .flatten()
        .flat_map(|variant| variant.iter().skip(STAGES - 2));
    let neck_layers = necks
        .iter()
        .flatten()
        .flatten()
        .flat_map(|(rim, core)| [rim, core]);
    let joins = bridges.iter().flatten().flatten().chain(neck_layers);
    for img in slimes.chain(joins) {
        for pixel in img.px.iter().filter(|p| p[3] > 0.05) {
            for under in SLIME_UNDERLAYS {
                let colour = mix(under, [pixel[0], pixel[1], pixel[2]], pixel[3]);
                samples.push(to_u8(colour));
            }
        }
    }

    // Score, tag and message strips are made at run time: sample black tape over the wall.
    let sample = dymo::strip("0123456789 YOUR MOVE", dymo::BLACK, 3, 0);
    for pixel in sample.px.iter().filter(|p| p[3] > 0.05) {
        for under in TAPE_UNDERLAYS {
            let colour = mix(under, [pixel[0], pixel[1], pixel[2]], pixel[3]);
            samples.push(to_u8(colour));
        }
    }

    let mut palette = [[0u8; 3]; 256];
    palette[..FIXED.len()].copy_from_slice(&FIXED);
    let cut = median_cut(samples, 256 - FIRST_FREE);
    palette[FIRST_FREE..FIRST_FREE + cut.len()].copy_from_slice(&cut);

    let mut quant = Quantizer::new(palette);
    let mut background = Bitmap::new(W as u32, H as u32);
    for (i, colour) in background_rgb.iter().enumerate() {
        background.data[i] = quant.get(*colour, i % W, i / W);
    }

    let colonies = sprites.map(|side| {
        side.iter()
            .map(|variant| {
                variant
                    .iter()
                    .map(|img| {
                        let mut bitmap = Bitmap::new(SPR as u32, SPR as u32);
                        for (i, pixel) in img.px.iter().enumerate() {
                            let (x, y) = (i % SPR, i / SPR);
                            // Dithered alpha: opaque where coverage beats the Bayer threshold.
                            if pixel[3] > bayer(x, y) {
                                bitmap.data[i] = quant.get([pixel[0], pixel[1], pixel[2]], x, y);
                            }
                        }
                        bitmap
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
        quant,
    }
}

/// Ordered-dither threshold for pixel `(x, y)`: the 4x4 Bayer matrix, tiled over the screen,
/// as 16 evenly spaced values in 0..1 (`(m + 0.5) / 16`, never exactly 0 or 1).
fn bayer(x: usize, y: usize) -> f32 {
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
    fn new(pal: [[u8; 3]; 256]) -> Self {
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
fn median_cut(samples: Vec<[u8; 3]>, count: usize) -> Vec<[u8; 3]> {
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
