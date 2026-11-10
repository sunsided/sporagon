//! Condensation: drops scattered over the wall and the tray, each a tiny lens.

use super::*;

/// A condensation drop on the wall or the tray, in screen pixels.
pub(super) struct Drop {
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
pub(super) fn droplets(
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
pub(super) fn apply_drop(color: [f32; 3], drop: &Drop, x: f32, y: f32) -> Option<[f32; 3]> {
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
