//! The egg tray: its height field, the knocked-out cups, cracks around them, its shading,
//! and the shadow it casts on the wall.

use super::drops::apply_drop;
use super::*;

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
pub(super) fn tray_height(x: f32, y: f32, centers: &[(f32, f32, bool)]) -> Option<(f32, usize)> {
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

/// Cracks spreading from the knocked-out cups, as a `W * H` mask: from each, 4 random walks of
/// 10..30 one-pixel steps, starting on a circle of 0.7 R around the cup centre.
pub(super) fn scatter_cracks(rng: &mut Rng, centers: &[(f32, f32, bool)]) -> Vec<bool> {
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
    cracks
}

/// Shade the tray at screen pixel `(fx, fy)`, which lies on cell `cell` at `height` (from
/// [`tray_height`]) and not inside a hole.
///
/// - Bright snapped-off plastic just around a hole, so holes read at a glance.
/// - Otherwise a surface normal from forward differences of [`tray_height`], Lambert and
///   Blinn lighting, grime in the cups and in broad patches, cracks, and drops on top.
pub(super) fn shade(
    fx: f32,
    fy: f32,
    (height, cell): (f32, usize),
    light: &Light,
    centers: &[(f32, f32, bool)],
    scatter: &Scatter,
) -> [f32; 3] {
    let (cracks, tray_drops) = (&scatter.cracks, &scatter.tray_drops);
    let (cell_x, cell_y, hole) = centers[cell];
    let hex_distance = hex_norm(fx - cell_x, fy - cell_y);
    if hole && hex_distance < hole_jag(fx, fy) + 0.07 {
        // The snapped-off plastic edge, bright so the hole reads at a glance.
        return scale([250.0, 246.0, 230.0], light.falloff.max(1.0) * 1.08);
    }

    // Surface normal from forward differences; off the tray, pretend a 4-pixel drop so
    // the outer edge still gets a slope.
    let step = 0.8;
    let slope_x = tray_height(fx + step, fy, centers).map_or(height - 4.0, |t| t.0) - height;
    let slope_y = tray_height(fx, fy + step, centers).map_or(height - 4.0, |t| t.0) - height;
    let (normal_x, normal_y, normal_z) = (-slope_x / step, -slope_y / step, 1.0);
    let length = (normal_x * normal_x + normal_y * normal_y + normal_z * normal_z).sqrt();
    let (normal_x, normal_y, normal_z) = (normal_x / length, normal_y / length, normal_z / length);

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
    color = scale(
        color,
        shade * light.falloff.min(1.15) * light.vignette.max(0.6),
    );
    color = mix(color, [255.0, 252.0, 236.0], specular * 0.9);

    if cracks[fy as usize * W + fx as usize] {
        color = scale(color, 0.45);
    }
    if let Some(dropped) = tray_drops.iter().find_map(|d| apply_drop(color, d, fx, fy)) {
        color = dropped;
    }
    color
}

/// The tray's drop shadow on the wall at `(fx, fy)`, cast 9 pixels right and 12 down (the
/// pixel is in shadow if the tray covers the spot 9 left and 12 up from it).
///
/// Where the pixel is seen down through a hole (`through` is its depth), the tray hangs
/// close above the wall, so the shadow edge is softened by averaging five taps.
pub(super) fn cast_shadow(
    mut color: [f32; 3],
    fx: f32,
    fy: f32,
    centers: &[(f32, f32, bool)],
    through: Option<f32>,
) -> [f32; 3] {
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
    color
}

/// Seen down through a hole at `depth` (from [`hole_depth`]): dimmer, darkest right under the
/// broken edge, with a thin black line where the plastic's cross-section is in its own
/// shadow.
pub(super) fn seen_through(mut color: [f32; 3], depth: f32) -> [f32; 3] {
    color = scale(color, 0.6 * (0.5 + 0.5 * smooth(0.0, 0.14, depth)));
    if depth < 0.025 {
        color = scale(color, 0.35);
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
pub(super) fn hole_depth(x: f32, y: f32, (cell_x, cell_y, hole): (f32, f32, bool)) -> Option<f32> {
    let depth = hole_jag(x, y) - hex_norm(x - cell_x, y - cell_y);
    (hole && depth > 0.0).then_some(depth)
}
