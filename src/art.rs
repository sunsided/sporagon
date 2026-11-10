//! All graphics are painted procedurally in true colour, then reduced to one 256-colour VGA
//! palette (median cut) with an ordered dither, the way 1990s artists prepared scanned art.
//!
//! Conventions shared by the painting modules: coordinates are pixels with y pointing down,
//! colours are RGB 0..255 as `f32` (values may stray outside and are clamped on quantising),
//! coverage (alpha) is 0..1, and the only light is the fridge bulb at the top right.

use game_toolkit_dosvideo::prelude::Bitmap;

pub mod dymo;
mod fridge;
mod noise;
mod palette;
mod shading;
pub mod slime;
mod spores;

pub use noise::Rng;
pub(crate) use noise::{fbm, hash, vnoise};
pub use palette::Quantizer;
use palette::{bayer, median_cut};
pub(crate) use shading::{facing, glint, lerp, mix, scale, smooth};

use fridge::paint_fridge;
pub(crate) use spores::MOULDS;

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
    let sprites: [Vec<Vec<Rgba>>; 2] = MOULDS.each_ref().map(|mould| {
        (0..VARIANTS as u32)
            .map(|variant| {
                (0..STAGES)
                    .map(|stage| {
                        let growth = (stage + 1) as f32 / STAGES as f32;
                        (mould.colony)(variant * mould.seed.0 + mould.seed.1, growth)
                    })
                    .collect()
            })
            .collect()
    });

    let mut background_rgb = paint_fridge(centers);

    // The labels that never change are stuck on the wall for good.
    for (text, tape, y) in [
        (GAME_TITLE, dymo::RED, TITLE_Y),
        (KEYS, dymo::BLACK, KEYS_Y),
    ]
    .into_iter()
    .chain(MOULDS.iter().zip(SIDE_Y).map(|(m, y)| (m.name, m.tape, y)))
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

    let necks: [[Vec<(Rgba, Rgba)>; 3]; 2] = MOULDS.each_ref().map(|mould| {
        BRIDGE_DIRS.map(|(q, r)| {
            let (x0, y0) = cell_center(0, 0);
            let (x1, y1) = cell_center(q, r);
            let offset = (x1 - x0, y1 - y0);
            (0..BRIDGE_VARIANTS as u32)
                .map(|variant| {
                    let seed = variant * 19 + 3;
                    (mould.neck)(offset, seed, BRIDGE)
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
