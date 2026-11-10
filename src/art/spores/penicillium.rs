//! Penicillium: velvety blue-green colonies with concentric zones, radial furrows, a white
//! advancing margin and amber guttation droplets, and the velvet necks that join them.

use super::*;

// Penicillium, shared by colonies and the necks between them so the two meet without a seam.

/// Penicillium's main velvet: dark blue-green conidia.
const VELVET: [f32; 3] = [52.0, 104.0, 90.0];
/// The lighter blue-green the velvet alternates with in zones and patches.
const VELVET2: [f32; 3] = [82.0, 132.0, 108.0];
/// Pale green band between the velvet and the white margin.
const RIM: [f32; 3] = [158.0, 196.0, 160.0];
/// White advancing margin of young, not yet sporulating hyphae.
const MARGIN: [f32; 3] = [238.0, 240.0, 228.0];
/// Greyish dusty bloom of loose conidia over the velvet.
const CONIDIA_DUST: [f32; 3] = [150.0, 176.0, 158.0];
/// The thin fuzz of single hyphae beyond the margin.
const FRINGE: [f32; 3] = [222.0, 228.0, 216.0];

/// Penicillium: velvety blue-green with concentric zones, radial furrows, a white advancing
/// margin of young hyphae, and amber guttation droplets on the older parts.
///
/// Returns an `SPR`-square sprite for one `seed` (variant) at `growth` 0..1. The colony
/// radius grows from 4 to 23 sprite pixels, so even the full colony with its fringe fits the
/// 64-pixel sprite.
///
/// - Per pixel, find the distance from the centre and the angle, and wobble the edge radius
///   with angular noise so the outline is lobed (`radius_frac` is 1 on that edge).
/// - Inside the edge, pick a base colour by ring: buff core, zoned velvet, pale rim, white
///   margin. Then add conidia dust, whiten young colonies, and shade by the dome, the radial
///   furrows and a fine grain. Finally sprinkle bright and dark specks of spore powder.
/// - Just outside the edge, up to 1.25, paint a thinning fuzz of young hyphae.
/// - On older colonies (growth > 0.6), add 2..=4 amber guttation droplets.
/// - Finish with `glisten`.
pub(in crate::art) fn penicillium(seed: u32, growth: f32) -> Rgba {
    let mut img = Rgba::new(SPR);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (seed as u64 * 7919));
    let center = SPR as f32 / 2.0;
    let colony_radius = 4.0 + 19.0 * growth;
    // 9, 11, 13 or 15 furrows around the colony, by variant.
    let furrows = 9.0 + (seed % 4) as f32 * 2.0;
    let rotation = rng.range(0.0, std::f32::consts::TAU);

    for y in 0..SPR {
        for x in 0..SPR {
            let (dx, dy) = (x as f32 + 0.5 - center, y as f32 + 0.5 - center);
            let distance = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx) + rotation;

            // Sampling noise on a circle keeps the wobble continuous all the way round.
            let edge_wobble = fbm(angle.cos() * 2.0 + 5.0, angle.sin() * 2.0 + 5.0, seed, 3);
            let edge_radius = colony_radius * (0.9 + 0.22 * edge_wobble);
            let radius_frac = distance / edge_radius;
            let fine_grain = fbm(x as f32 / 1.7, y as f32 / 1.7, seed + 11, 2);

            if radius_frac < 1.0 {
                // Base colour by ring, from the oldest centre outwards.
                let core = [118.0, 132.0, 104.0];
                let zone = (radius_frac * 7.0 + edge_wobble * 2.0).sin() * 0.5 + 0.5;
                let mut color = if radius_frac < 0.3 {
                    mix(core, VELVET, radius_frac / 0.3)
                } else if radius_frac < 0.72 {
                    mix(VELVET, VELVET2, zone)
                } else if radius_frac < 0.88 {
                    mix(VELVET2, RIM, (radius_frac - 0.72) / 0.16)
                } else {
                    mix(RIM, MARGIN, (radius_frac - 0.88) / 0.12)
                };

                // Dusty bloom of conidia.
                let dust = smooth(0.55, 0.8, fbm(x as f32 / 4.0, y as f32 / 4.0, seed + 13, 3));
                let inside_velvet = (radius_frac < 0.85) as u8 as f32;
                color = mix(color, CONIDIA_DUST, dust * 0.5 * inside_velvet);

                // Young colonies are still mostly white mycelium.
                let youth = (1.0 - growth * 1.4).max(0.0);
                color = mix(color, MARGIN, youth * (1.0 - radius_frac * 0.3));

                // Shading: dome, radial furrows (deeper outwards and with age), fine grain.
                let furrow = ((angle * furrows + edge_wobble * 4.0).sin() * 0.5 + 0.5).powi(6);
                let furrow_shade = 1.0 - 0.25 * furrow * (radius_frac * 2.0).min(1.0) * growth;
                let brightness = dome(dx / edge_radius, dy / edge_radius, radius_frac)
                    * furrow_shade
                    * (0.84 + 0.32 * fine_grain);
                color = scale(color, brightness);

                // Sporulating powder: bright and dark specks.
                let speck = hash(x as i32, y as i32, seed + 3);
                if radius_frac < 0.85 && speck > 0.93 {
                    color = scale(color, 1.25);
                } else if radius_frac < 0.85 && speck < 0.05 {
                    color = scale(color, 0.7);
                }

                img.over(x as i32, y as i32, color, 1.0);
            } else if radius_frac < 1.25 {
                // Fuzzy fringe of young hyphae, thinning out.
                let fuzz = fbm(x as f32 / 1.3, y as f32 / 1.3, seed + 7, 2);
                let alpha = (1.0 - (radius_frac - 1.0) / 0.25).powf(1.5) * smooth(0.3, 0.7, fuzz);
                img.over(x as i32, y as i32, FRINGE, alpha * 0.85);
            }
        }
    }

    // Guttation: little amber droplets of exudate.
    if growth > 0.6 {
        for _ in 0..(seed % 3 + 2) {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            // On the velvet, clear of the centre and the margin.
            let distance = rng.range(0.2, 0.6) * colony_radius;
            let (drop_x, drop_y) = (
                center + angle.cos() * distance,
                center + angle.sin() * distance,
            );
            let drop_radius = rng.range(1.0, 2.2);

            img.disk(
                drop_x + 0.6,
                drop_y + 0.8,
                drop_radius,
                [40.0, 50.0, 30.0],
                0.5,
            );
            img.disk(drop_x, drop_y, drop_radius, [196.0, 150.0, 60.0], 1.0);
            img.over(
                (drop_x + drop_radius * 0.4) as i32,
                (drop_y - drop_radius * 0.5) as i32,
                [255.0, 250.0, 220.0],
                1.0,
            );
        }
    }

    glisten(&mut img, seed, colony_radius, growth);
    img
}

/// Penicillium's join as `(rim, core)`: draw every rim first and every core after, so the
/// cores cover the margins where joins cross inside a mass.
///
/// `v` is the vector between the two cell centres and `size` the square sprite size, both in
/// sprite pixels. Pixels with `radius_frac` (distance across over half-width) below 0.78
/// go to the core, the rest and the fringe to the rim. Everything is scaled by the neck's
/// fade so both ends blend into the colonies.
///
/// - Per pixel, roughen `radius_frac` with fine noise so the edge is ragged.
/// - Inside the edge: patchy velvet, a ragged pale rim and white margin starting at a noisy
///   0.66..0.78, conidia dust on the inner part, then shading by a dome across the neck
///   (lit on the flank facing the bulb), a fine grain and buckling wrinkles, and finally
///   the same spore specks as the colony.
/// - Just outside, up to 1.25: the same thinning fringe as the colony.
pub(in crate::art) fn penicillium_neck(v: (f32, f32), seed: u32, size: usize) -> (Rgba, Rgba) {
    let mut rim_img = Rgba::new(size);
    let mut img = Rgba::new(size);
    let neck = Neck::new(v, size);

    for y in 0..size {
        for x in 0..size {
            let sample = neck.at(x, y, seed);
            if sample.fade <= 0.0 {
                continue;
            }

            // Ragged edge, pixel by pixel.
            let ragged = fbm(x as f32 / 2.5, y as f32 / 2.5, seed + 21, 2) - 0.5;
            let radius_frac = sample.across.abs() / sample.half_width + 0.16 * ragged;
            let fine_grain = fbm(x as f32 / 1.7, y as f32 / 1.7, seed + 11, 2);

            if radius_frac < 1.0 {
                let layer = if radius_frac < 0.78 {
                    &mut img
                } else {
                    &mut rim_img
                };

                // Powdery velvet in patches, no pattern.
                let patch = fbm(x as f32 / 5.0, y as f32 / 5.0, seed + 2, 3);
                let mut color = mix(VELVET, VELVET2, smooth(0.3, 0.7, patch));

                // The white margin is ragged and wider where the edge bulges.
                let margin_start =
                    0.66 + 0.12 * fbm(sample.along / 4.0, sample.across.signum(), seed + 9, 2);
                if radius_frac > margin_start {
                    color = mix(
                        color,
                        RIM,
                        smooth(margin_start, margin_start + 0.12, radius_frac),
                    );
                    color = mix(color, MARGIN, smooth(margin_start + 0.12, 1.0, radius_frac));
                }

                let dust = smooth(0.5, 0.8, fbm(x as f32 / 3.5, y as f32 / 3.5, seed + 13, 3));
                color = mix(
                    color,
                    CONIDIA_DUST,
                    dust * 0.55 * (1.0 - smooth(0.6, 0.85, radius_frac)),
                );

                // Fine wrinkles where the mat buckled as it grew over the ridge.
                let wrinkle = (fbm(x as f32 / 6.0, y as f32 / 6.0, seed + 17, 3) - 0.5).abs();
                // Signed offset across the neck in units of its half-width, as a dome offset.
                let side_frac = sample.across.signum() * radius_frac.min(1.0);
                let brightness = dome(
                    neck.normal.0 * side_frac,
                    neck.normal.1 * side_frac,
                    radius_frac.min(1.0),
                ) * (0.84 + 0.32 * fine_grain)
                    * (0.82 + 0.18 * smooth(0.0, 0.06, wrinkle));
                color = scale(color, brightness);

                // Sporulating powder: bright and dark specks.
                let speck = hash(x as i32, y as i32, seed + 3);
                if radius_frac < 0.8 && speck > 0.93 {
                    color = scale(color, 1.25);
                } else if radius_frac < 0.8 && speck < 0.05 {
                    color = scale(color, 0.7);
                }

                layer.over(x as i32, y as i32, color, sample.fade);
            } else if radius_frac < 1.25 {
                let fuzz = fbm(x as f32 / 1.3, y as f32 / 1.3, seed + 7, 2);
                let alpha = (1.0 - (radius_frac - 1.0) / 0.25).powf(1.5) * smooth(0.3, 0.7, fuzz);
                rim_img.over(x as i32, y as i32, FRINGE, alpha * 0.85 * sample.fade);
            }
        }
    }

    (rim_img, img)
}
