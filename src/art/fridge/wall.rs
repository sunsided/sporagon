//! The fridge's back wall: yellowed plastic, shelf rails, ribbed side walls, brown stains and
//! drips, and black mildew in the damp places.

use super::*;

/// Shelf rails across the back wall: the y of each rail's top edge, in screen pixels.
const RAILS: [f32; 3] = [118.0, 300.0, 452.0];

/// A brown drip running down the back wall, in screen pixels.
pub(super) struct Drip {
    x: f32,
    /// Where it starts.
    top: f32,
    length: f32,
    /// Width at the top; it thins towards its end.
    width: f32,
}

/// Scatter 11 drips. Draws x, top, length, width from `rng` per drip, in that order.
pub(super) fn scatter_drips(rng: &mut Rng) -> Vec<Drip> {
    (0..11)
        .map(|_| Drip {
            x: rng.range(30.0, 620.0),
            top: rng.range(0.0, 200.0),
            length: rng.range(60.0, 300.0),
            width: rng.range(1.5, 4.0),
        })
        .collect()
}

/// The unlit wall colour at screen pixel `(fx, fy)`, layer by layer.
pub(super) fn paint(fx: f32, fy: f32, drips: &[Drip]) -> [f32; 3] {
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
    for drip in drips {
        let (drip_x, drip_y, drip_length, drip_width) = (drip.x, drip.top, drip.length, drip.width);
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
    color
}
