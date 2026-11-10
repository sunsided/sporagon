//! The rundown fridge: back wall, egg tray, bulb and grime.
//!
//! Everything here is in screen pixels on the `W` x `H` (640 x 480) background, origin top
//! left, y down. Colours are RGB 0..255 as `f32`, unclamped; quantising to the palette
//! happens later. The tray is described by `centers`: one `(x, y, hole)` per board cell,
//! where `hole` marks a knocked-out cup through which the back wall shows.

use super::*;

/// Shelf rails across the back wall: the y of each rail's top edge, in screen pixels.
const RAILS: [f32; 3] = [118.0, 300.0, 452.0];

/// A condensation drop on the wall or the tray, in screen pixels.
struct Drop {
    /// Centre x.
    x: f32,
    /// Centre y.
    y: f32,
    /// Radius.
    r: f32,
}

/// Scatter `count` drops uniformly over the rectangle `(x0, y0)..(x1, y1)`, in screen pixels.
///
/// Radii fall in 0.48..1.0 of `max_radius`; the product of two uniform draws skews them
/// towards the small end. Draws three numbers from `rng` per drop, in the order x, y, radius.
fn droplets(
    rng: &mut Rng,
    count: usize,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    max_radius: f32,
) -> Vec<Drop> {
    (0..count)
        .map(|_| Drop {
            x: rng.range(x0, x1),
            y: rng.range(y0, y1),
            r: rng.range(0.8, 1.0) * rng.range(0.6, 1.0) * max_radius,
        })
        .collect()
}

/// A condensation drop acts as a tiny lens: darker rim, bright specular toward the bulb.
///
/// Returns the colour of screen pixel `(x, y)` seen through `drop`, given the colour `color`
/// underneath, or `None` if the pixel lies outside the drop.
///
/// - Work in units of the drop radius, so the drop is the unit disk.
/// - A small spot up right of the centre (squared distance < 0.07 from (0.35, -0.4), i.e.
///   towards the bulb) is a flat near-white highlight.
/// - Elsewhere the drop darkens towards its rim, and its lower part (dy > 0.2) is brighter
///   than the upper part: the lens focuses the light from above onto the far side.
fn apply_drop(color: [f32; 3], drop: &Drop, x: f32, y: f32) -> Option<[f32; 3]> {
    let (dx, dy) = ((x - drop.x) / drop.r, (y - drop.y) / drop.r);
    let distance_sq = dx * dx + dy * dy;
    if distance_sq > 1.0 {
        return None;
    }

    let specular = ((dx - 0.35).powi(2) + (dy + 0.4).powi(2)) < 0.07;
    if specular {
        return Some([255.0, 255.0, 245.0]);
    }

    let rim = smooth(0.55, 1.0, distance_sq);
    // Light refracted through the lower half.
    let lens = if dy > 0.2 { 1.12 } else { 0.92 };
    Some(scale(color, lens * (1.0 - 0.35 * rim)))
}

/// Height of the egg tray (z up, in pixels) or `None` outside it.
///
/// Returns `(height, cell_index)` for screen pixel `(x, y)`, where `cell_index` is the
/// nearest entry in `centers`. The flat deck is at 0, the cup bottoms at -14.5 and the outer
/// edge slopes down to -7; anything further than `1.06 * R` from every cell centre is off the
/// tray. Holes are not cut out here; see `hole_depth`.
///
/// - Find the nearest cell centre by brute force (squared distance, first one wins ties).
/// - The deck is flat at -2.5 inside the cell and rises to 0 on the ridge along the hex
///   edges (hexagonal distance 0.78..1).
/// - A round cup of radius `0.8 * R` is pressed 12 pixels into it; the 0.8 exponent makes
///   its walls steeper than a sphere's.
/// - Past `0.95 * R` from the nearest centre, mostly along the tray's outer border (and a
///   slight dip at the corners where three cells meet), the surface drops by up to 7 pixels.
fn tray_height(x: f32, y: f32, centers: &[(f32, f32, bool)]) -> Option<(f32, usize)> {
    let mut nearest = (f32::MAX, 0);
    for (i, &(cell_x, cell_y, _)) in centers.iter().enumerate() {
        let distance_sq = (x - cell_x).powi(2) + (y - cell_y).powi(2);
        if distance_sq < nearest.0 {
            nearest = (distance_sq, i);
        }
    }

    let distance = nearest.0.sqrt();
    if distance > R * 1.06 {
        return None;
    }

    let (cell_x, cell_y, _) = centers[nearest.1];
    let hex_distance = hex_norm(x - cell_x, y - cell_y);

    // A round cup pressed into a flat deck that rises to a ridge on the hex edges.
    let deck = -2.5 * (1.0 - smooth(0.78, 1.0, hex_distance));
    let cup_frac = distance / (R * 0.8);
    let cup = if cup_frac < 1.0 {
        -12.0 * (1.0 - cup_frac * cup_frac).powf(0.8)
    } else {
        0.0
    };
    let edge = smooth(R * 0.95, R * 1.06, distance);

    Some((deck + cup - edge * 7.0, nearest.1))
}

/// Paint the whole fridge background: one RGB colour (0..255 as `f32`) per screen pixel,
/// row by row, `W * H` entries.
///
/// `centers` lists the board cells as `(x, y, hole)` in screen pixels. The result is fully
/// deterministic: the RNG has a fixed seed.
///
/// - Scatter, in this fixed RNG order: 11 brown drips on the back wall, 160 wall drops, 140
///   drops over the tray's bounding box, and 4 random-walk cracks from each knocked-out cup.
/// - Rasterise the cracks into a per-pixel mask.
/// - Shade every pixel with `fridge_pixel`.
pub(super) fn paint_fridge(centers: &[(f32, f32, bool)]) -> Vec<[f32; 3]> {
    let mut rng = Rng(0x5EED_F00D_CAFE_1234);
    let bulb = BULB;

    // Brown drips running down the back wall: (x, start y, length, width), in pixels.
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
    // The box covers the tray around the board centre (BX, BY).
    let tray_drops = droplets(
        &mut rng,
        140,
        BX - 240.0,
        BY - 215.0,
        BX + 240.0,
        BY + 215.0,
        2.6,
    );

    // Cracks spreading from the knocked-out cups: random walks of 10..30 one-pixel steps,
    // starting on a circle of 0.7 R around the cup centre.
    let mut cracks = vec![false; W * H];
    for &(cell_x, cell_y, hole) in centers {
        if !hole {
            continue;
        }
        for _ in 0..4 {
            let mut heading = rng.range(0.0, std::f32::consts::TAU);
            let (mut x, mut y) = (
                cell_x + heading.cos() * R * 0.7,
                cell_y + heading.sin() * R * 0.7,
            );

            let steps = rng.range(10.0, 30.0) as u32;
            for _ in 0..steps {
                heading += rng.range(-0.5, 0.5);
                x += heading.cos();
                y += heading.sin();
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
                centers,
                &cracks,
            );
        }
    }
    out
}

/// The colour of one background pixel at screen position `(fx, fy)`, RGB 0..255 as `f32`.
///
/// `bulb` is the bulb position, `drips` the wall drips as `(x, start y, length, width)`,
/// `cracks` a `W * H` mask, and `centers` the board cells; all in screen pixels. `(fx, fy)`
/// must be on screen, since it indexes `cracks`.
///
/// - Light: a falloff from the bulb (0.62 far away, up to 1.32 at the bulb) and an
///   elliptical vignette around the screen centre (1 there, about 0.43 in the corners;
///   each layer clamps it from below).
/// - Tray: if the pixel is on the tray and not in a hole, shade the tray and return:
///   bright snapped-off edges around holes, otherwise a surface normal from finite
///   differences of `tray_height`, Lambert and Blinn lighting, grime in the cups and in
///   broad patches, cracks, and tray drops.
/// - Back wall, painted in layers: yellowed plastic, shelf rails, ribbed side walls, stains
///   and drips, black mildew, lighting, the drop shadow of the tray, the thermostat knob,
///   the bulb and its glow, wall drops, and finally the dimming seen through a hole.
#[allow(clippy::too_many_arguments)]
fn fridge_pixel(
    fx: f32,
    fy: f32,
    bulb: (f32, f32),
    drips: &[(f32, f32, f32, f32)],
    wall_drops: &[Drop],
    tray_drops: &[Drop],
    centers: &[(f32, f32, bool)],
    cracks: &[bool],
) -> [f32; 3] {
    // Lighting shared by the tray and the wall.
    let dist_bulb = ((fx - bulb.0).powi(2) + (fy - bulb.1).powi(2)).sqrt();
    let light = 0.62 + 0.7 * (-dist_bulb / 380.0).exp();
    // 1 at the screen centre (320, 240), about 0.7 mid-edge and 0.43 in the corners.
    let vignette = 1.0 - 0.35 * (((fx - 320.0) / 340.0).powi(2) + ((fy - 240.0) / 280.0).powi(2));

    // Tray first: it sits in front of everything but the notes. Through a knocked-out cup
    // the fridge wall behind shows.
    let tray = tray_height(fx, fy, centers);
    let through = tray.and_then(|(_, cell)| hole_depth(fx, fy, centers[cell]));
    if let Some((height, cell)) = tray
        && through.is_none()
    {
        let (cell_x, cell_y, hole) = centers[cell];
        let hex_distance = hex_norm(fx - cell_x, fy - cell_y);
        if hole && hex_distance < hole_jag(fx, fy) + 0.07 {
            // The snapped-off plastic edge, bright so the hole reads at a glance.
            return scale([250.0, 246.0, 230.0], light.max(1.0) * 1.08);
        }

        // Surface normal from forward differences; off the tray, pretend a 4-pixel drop so
        // the outer edge still gets a slope.
        let step = 0.8;
        let slope_x = tray_height(fx + step, fy, centers).map_or(height - 4.0, |t| t.0) - height;
        let slope_y = tray_height(fx, fy + step, centers).map_or(height - 4.0, |t| t.0) - height;
        let (normal_x, normal_y, normal_z) = (-slope_x / step, -slope_y / step, 1.0);
        let length = (normal_x * normal_x + normal_y * normal_y + normal_z * normal_z).sqrt();
        let (normal_x, normal_y, normal_z) =
            (normal_x / length, normal_y / length, normal_z / length);

        let diffuse = facing([normal_x, normal_y, normal_z]).max(0.0);
        let specular = glint([normal_x, normal_y, normal_z]).max(0.0).powi(40);
        let mut color = [228.0, 222.0, 200.0];

        // Grime settles in the bottom of the cups.
        let settle = ((1.0 - hex_distance).max(0.0)).powi(2) * fbm(fx / 7.0, fy / 7.0, 21, 3);
        color = mix(color, [150.0, 124.0, 74.0], settle * 0.7);
        // Broad yellowed patches across the whole tray.
        color = mix(
            color,
            [190.0, 176.0, 130.0],
            smooth(0.5, 0.8, fbm(fx / 40.0, fy / 40.0, 22, 4)) * 0.6,
        );

        let shade = 0.5 + 0.62 * diffuse;
        color = scale(color, shade * light.min(1.15) * vignette.max(0.6));
        color = mix(color, [255.0, 252.0, 236.0], specular * 0.9);

        if cracks[fy as usize * W + fx as usize] {
            color = scale(color, 0.45);
        }
        if let Some(dropped) = tray_drops.iter().find_map(|d| apply_drop(color, d, fx, fy)) {
            color = dropped;
        }
        return color;
    }

    // Back wall: yellowed plastic, a darker band where the shelf rails run.
    let discolour = fbm(fx / 90.0, fy / 90.0, 1, 4);
    let fine_grain = fbm(fx / 3.0, fy / 3.0, 2, 2);
    let mut color = mix([226.0, 220.0, 190.0], [198.0, 184.0, 136.0], discolour);
    color = scale(color, 0.92 + 0.12 * fine_grain);

    // Each rail: a 3-pixel bright lip, then a 6-pixel shadow lightening downwards.
    for rail in RAILS {
        let below = fy - rail;
        if (0.0..3.0).contains(&below) {
            color = scale(color, 1.18);
        } else if (3.0..9.0).contains(&below) {
            color = scale(color, 0.62 + 0.05 * (below - 3.0));
        }
    }

    // Side walls in perspective, ribbed. `inset` is the distance from the screen edge; the
    // walls get slightly wider towards the bottom.
    let side = if fx < 34.0 - fy * 0.02 {
        Some(fx)
    } else if fx > 606.0 + fy * 0.02 {
        Some(W as f32 - fx)
    } else {
        None
    };
    if let Some(inset) = side {
        color = scale(color, 0.62 + inset * 0.006);
        // Slanted ribs every 26 pixels, 3 pixels thick.
        if ((fy + inset * 1.5) % 26.0) < 3.0 {
            color = scale(color, 0.75);
        }
    }

    // Brown stains and drips.
    let stain = smooth(0.58, 0.8, fbm(fx / 55.0, fy / 35.0, 7, 5));
    color = mix(color, [150.0, 112.0, 60.0], stain * 0.55);
    for &(drip_x, drip_y, drip_length, drip_width) in drips {
        let progress = (fy - drip_y) / drip_length;
        if (0.0..1.0).contains(&progress) {
            // The drip meanders a little and thins towards its end, where it beads up.
            let center_x = drip_x + (fy / 23.0).sin() * 1.5 + fbm(fy / 30.0, drip_x, 9, 2) * 4.0;
            let bead = if progress > 0.93 { 1.5 } else { 0.0 };
            let width = drip_width * (1.0 - progress * 0.7) + bead;
            let distance = (fx - center_x).abs();
            if distance < width {
                color = mix(color, [128.0, 88.0, 40.0], 0.55 + 0.3 * (distance / width));
            }
        }
    }

    // Black mildew (Cladosporium) colonising the damp corners and the rails.
    // `corner` is 1 in a corner, falling off over about 110 pixels (bottom left, bottom
    // right, top left; the bulb keeps the top right dry).
    let corner = [(0.0, 480.0), (640.0, 480.0), (0.0, 0.0)]
        .iter()
        .map(|&(corner_x, corner_y): &(f32, f32)| {
            (-((fx - corner_x).powi(2) + (fy - corner_y).powi(2)).sqrt() / 110.0).exp()
        })
        .fold(0.0f32, f32::max);
    // Damp in the rail shadow, 6 pixels below each rail's top.
    let railness = RAILS
        .iter()
        .map(|r| (-(fy - r - 6.0).abs() / 6.0).exp())
        .fold(0.0, f32::max)
        * 0.5;
    let mildew = (corner + railness) * fbm(fx / 14.0, fy / 14.0, 31, 3);
    // Speckle: the damper the spot, the more of the noise passes the threshold.
    let spot = vnoise(fx / 1.8, fy / 1.8, 32);
    if spot > 1.0 - mildew * 0.9 {
        color = mix(color, [26.0, 30.0, 24.0], 0.75 + 0.25 * spot);
    } else if spot > 1.0 - mildew * 1.3 {
        color = mix(color, [70.0, 72.0, 52.0], 0.4);
    }

    color = scale(color, light * vignette.max(0.35));

    // Drop shadow of the tray and the notes, cast 9 pixels left and 12 down from the
    // top-right light.
    let casts = |sx: f32, sy: f32| {
        tray_height(sx, sy, centers)
            .is_some_and(|(_, cell)| hole_depth(sx, sy, centers[cell]).is_none())
    };
    // Down in a hole the tray is close above the wall: soften its shadow edge by averaging
    // five taps around the shadow sample.
    let shade_in_hole = through.map(|_| {
        let taps = [
            (0.0, 0.0),
            (2.5, 1.0),
            (-1.0, 2.5),
            (-2.5, -1.0),
            (1.0, -2.5),
        ];
        taps.iter()
            .filter(|(ox, oy)| casts(fx - 9.0 + ox, fy - 12.0 + oy))
            .count() as f32
            / taps.len() as f32
    });
    if shade_in_hole.is_none() && casts(fx - 9.0, fy - 12.0) {
        color = scale(color, 0.58);
    }
    if let Some(shadow) = shade_in_hole {
        color = scale(color, 1.0 - 0.45 * shadow);
    }

    // The thermostat knob, stuck on 7 since 1987. Centred at (52, 52), radius 21, with a
    // 3-pixel shadow ring around it.
    let (knob_x, knob_y) = (fx - 52.0, fy - 52.0);
    let knob_distance = (knob_x * knob_x + knob_y * knob_y).sqrt();
    if knob_distance < 21.0 {
        let angle = knob_y.atan2(knob_x);
        // 18 ridges around the outer ring.
        let grip = if knob_distance > 15.0 && (angle * 18.0).sin() > 0.3 {
            0.8
        } else {
            1.0
        };
        // A shallow dome lit from the top right.
        let dome =
            1.05 - -(0.4 * ((knob_x - 6.0) * 0.6 / 21.0 + (knob_y + 6.0) * 0.8 / 21.0).max(-1.0));
        color = scale([214.0, 210.0, 196.0], dome.clamp(0.5, 1.3) * grip * light);

        // The pointer, at 7: a 3.2-pixel bar from 2 to 14 pixels out, at -0.8 rad.
        let (pointer_x, pointer_y) = ((-0.8f32).cos(), (-0.8f32).sin());
        let along = knob_x * pointer_x + knob_y * pointer_y;
        if (knob_x * pointer_y - knob_y * pointer_x).abs() < 1.6 && (2.0..14.0).contains(&along) {
            color = [150.0, 30.0, 24.0];
        }

        if knob_distance > 20.0 {
            color = scale(color, 0.5);
        }
        color = mix(
            color,
            [120.0, 96.0, 50.0],
            fbm(fx / 4.0, fy / 4.0, 71, 2) * 0.35,
        );
    } else if knob_distance < 24.0 {
        color = scale(color, 0.6);
    }

    // The bulb in its cage, rusted at the mount: glass inside 15 pixels with a cage bar
    // every 7 pixels, a rusty mount ring out to 22, and a wide glow.
    let glow = (-dist_bulb / 60.0).exp() * 0.5;
    color = mix(color, [255.0, 240.0, 190.0], glow);
    if dist_bulb < 15.0 {
        let core = 1.0 - dist_bulb / 15.0;
        color = mix([255.0, 214.0, 120.0], [255.0, 255.0, 236.0], core * 1.4);
        if ((fx - bulb.0) as i32).rem_euclid(7) == 0 && dist_bulb > 4.0 {
            color = [70.0, 62.0, 50.0];
        }
    } else if dist_bulb < 22.0 {
        color = mix(
            [120.0, 70.0, 36.0],
            [62.0, 40.0, 26.0],
            fbm(fx / 2.0, fy / 2.0, 61, 2),
        );
    }

    // Condensation on the wall.
    if let Some(dropped) = wall_drops.iter().find_map(|d| apply_drop(color, d, fx, fy)) {
        color = dropped;
    }

    // Seen through a hole in the tray.
    if let Some(depth) = through {
        // Seen down through the tray: dimmer, darkest right under the broken edge, with a
        // thin black line where the plastic's cross-section is in its own shadow.
        color = scale(color, 0.6 * (0.5 + 0.5 * smooth(0.0, 0.14, depth)));
        if depth < 0.025 {
            color = scale(color, 0.35);
        }
    }

    color
}

/// The ragged outline of a knocked-out cup, in hexagonal distance from its centre.
///
/// Returns 0.78..0.94 (fbm is 0..1), so the hole stays inside the cell and leaves a ring of
/// deck around it. Depends only on the screen position, so neighbouring pixels agree.
fn hole_jag(x: f32, y: f32) -> f32 {
    0.78 + 0.16 * fbm(x / 5.0, y / 5.0, 77, 2)
}

/// How far inside the opening of a knocked-out cup `(x, y)` lies, if it does.
///
/// `cell` is that cell's `(x, y, hole)`, in screen pixels. Returns the margin by which the
/// pixel's hexagonal distance from the centre is inside the jagged outline (0 on it, up to
/// about 0.94 at the centre), or `None` if the cup is intact or the pixel is outside.
fn hole_depth(x: f32, y: f32, (cell_x, cell_y, hole): (f32, f32, bool)) -> Option<f32> {
    let depth = hole_jag(x, y) - hex_norm(x - cell_x, y - cell_y);
    (hole && depth > 0.0).then_some(depth)
}
