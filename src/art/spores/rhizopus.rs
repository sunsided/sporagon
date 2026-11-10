//! Rhizopus: a tangle of white cottony hyphae with sporangia ripening from white to black,
//! and the cotton necks that join neighbouring colonies.

use super::*;

/// Rhizopus cotton at sprite pixel `(x, y)`, `radius_frac` (0 on the axis, 1 at the edge)
/// from the colony centre or the neck axis: `(density, colour)` before lighting.
///
/// Density is 0 at and beyond `radius_frac` 1 and up to about 1.35 inside; callers treat
/// `density > 0` as inside and map it to alpha with `smooth(0.08, 0.4, density)`. The colour
/// is a warm white to grey in 0..255, brighter where the fluff noise is high.
fn cotton(x: usize, y: usize, seed: u32, radius_frac: f32) -> (f32, [f32; 3]) {
    let fluff = fbm(x as f32 / 2.3, y as f32 / 2.3, seed + 5, 3);
    let density = (1.0 - radius_frac * radius_frac).max(0.0) * (0.45 + 0.9 * fluff);
    let grey = lerp(150.0, 246.0, fluff.powf(0.8));

    (density, [grey, grey * 0.99, grey * 0.94])
}

/// Rhizopus, black bread mould: a tangle of white cottony hyphae dotted with sporangia that
/// start white and turn black as they ripen.
///
/// Returns an `SPR`-square sprite for one `seed` (variant) at `growth` 0..1. The colony
/// radius grows from 4 to 24 sprite pixels; stray hyphae may reach a little past it.
///
/// - Per pixel, wobble the edge radius with angular noise and paint `cotton` there, greyed
///   in the old centre and shaded by the dome.
/// - Draw 20..90 aerial hyphae as random walks of 0.7-pixel steps from inside 60 % of the
///   radius, fainter where they stray past the edge.
/// - Draw up to 44 sporangia (quadratic in growth, so they appear late), each with a slanted
///   stalk, a shadow, a white or ripe black head, and a highlight towards the bulb.
/// - Finish with `glisten`.
pub(in crate::art) fn rhizopus(seed: u32, growth: f32) -> Rgba {
    let mut img = Rgba::new(SPR);
    let mut rng = Rng(0xD1B5_4A32_D192_ED03 ^ (seed as u64 * 104_729));
    let center = SPR as f32 / 2.0;
    let colony_radius = 4.0 + 20.0 * growth;

    for y in 0..SPR {
        for x in 0..SPR {
            let (dx, dy) = (x as f32 + 0.5 - center, y as f32 + 0.5 - center);
            let distance = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);
            let edge_wobble = fbm(angle.cos() * 1.6 + 9.0, angle.sin() * 1.6 + 9.0, seed, 3);
            let radius_frac = distance / (colony_radius * (0.85 + 0.3 * edge_wobble));

            let (density, mut color) = cotton(x, y, seed, radius_frac);
            if density > 0.0 {
                // Old centre: grey from spores that already fell.
                let fallen_spores = (1.0 - radius_frac * 1.6).max(0.0) * growth * 0.8;
                color = mix(color, [96.0, 94.0, 90.0], fallen_spores);
                color = scale(
                    color,
                    dome(dx / colony_radius, dy / colony_radius, radius_frac.min(1.0)),
                );
                img.over(x as i32, y as i32, color, smooth(0.08, 0.4, density));
            }
        }
    }

    // Aerial hyphae: wandering strands, past the colony edge too.
    let strand_count = (20.0 + 70.0 * growth) as u32;
    for _ in 0..strand_count {
        let start_angle = rng.range(0.0, std::f32::consts::TAU);
        let start_distance = rng.range(0.0, 0.6) * colony_radius;
        let (mut x, mut y) = (
            center + start_angle.cos() * start_distance,
            center + start_angle.sin() * start_distance,
        );
        // Start roughly outwards, then wander.
        let mut heading = start_angle + rng.range(-0.8, 0.8);
        let length = rng.range(0.5, 1.0) * colony_radius * 1.1;

        let mut travelled = 0.0;
        while travelled < length {
            heading += rng.range(-0.35, 0.35);
            x += heading.cos() * 0.7;
            y += heading.sin() * 0.7;
            travelled += 0.7;

            let grey = rng.range(205.0, 245.0);
            let radius_frac = ((x - center).powi(2) + (y - center).powi(2)).sqrt() / colony_radius;
            img.over(
                x as i32,
                y as i32,
                [grey, grey, grey * 0.96],
                if radius_frac > 0.95 { 0.35 } else { 0.6 },
            );
        }
    }

    // Sporangia on their stalks.
    let sporangium_count = (44.0 * growth * growth) as u32;
    for _ in 0..sporangium_count {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let distance = rng.f().sqrt() * 0.9 * colony_radius;
        let (head_x, head_y) = (
            center + angle.cos() * distance,
            center + angle.sin() * distance,
        );
        let head_radius = rng.range(0.9, 2.2);
        let ripe = rng.f() < growth * 0.95;

        // The stalk seen at a slant, and its shadow.
        for step in 0..3 {
            img.over(
                (head_x - 0.5 - step as f32 * 0.4) as i32,
                (head_y + 1.0 + step as f32) as i32,
                [176.0, 172.0, 160.0],
                0.7,
            );
        }
        img.disk(
            head_x + 1.2,
            head_y + 1.5,
            head_radius,
            [50.0, 48.0, 44.0],
            0.35,
        );

        let body = if ripe {
            [22.0, 22.0, 26.0]
        } else {
            [214.0, 210.0, 196.0]
        };
        img.disk(head_x, head_y, head_radius, body, 1.0);

        let highlight = if ripe {
            [120.0, 124.0, 130.0]
        } else {
            [255.0, 255.0, 250.0]
        };
        img.over(
            (head_x + head_radius * 0.35) as i32,
            (head_y - head_radius * 0.45) as i32,
            highlight,
            0.9,
        );
    }

    glisten(&mut img, seed, colony_radius, growth);
    img
}

/// Rhizopus' join as `(rim, core)`; cotton has no margin to hide, so the rim is empty.
///
/// `v` is the vector between the two cell centres and `size` the square sprite size, both in
/// sprite pixels.
///
/// - Per pixel, paint `cotton` with `radius_frac` measured across the neck, shaded by a dome
///   across it and faded towards both colonies.
/// - Draw 45 hyphae as random walks of 8..22 steps of 0.7 pixels, mostly along the axis in
///   a random direction, starting anywhere within `reach` and ±9 pixels of the axis.
/// - Draw 12 ripe sporangia, kept 6 pixels clear of the faded ends.
pub(in crate::art) fn rhizopus_neck(v: (f32, f32), seed: u32, size: usize) -> (Rgba, Rgba) {
    let mut img = Rgba::new(size);
    let neck = Neck::new(v, size);
    let mut rng = Rng(0x7F4A_7C15_D1B5_4A32 ^ (seed as u64 * 2_654_435_761));

    for y in 0..size {
        for x in 0..size {
            let sample = neck.at(x, y, seed);
            let radius_frac = sample.across.abs() / sample.half_width;
            let (density, color) = cotton(x, y, seed, radius_frac);

            if density > 0.0 && sample.fade > 0.0 {
                let side_frac = sample.across.signum() * radius_frac.min(1.0);
                let color = scale(
                    color,
                    dome(
                        neck.normal.0 * side_frac,
                        neck.normal.1 * side_frac,
                        radius_frac.min(1.0),
                    ),
                );
                img.over(
                    x as i32,
                    y as i32,
                    color,
                    smooth(0.08, 0.4, density) * sample.fade,
                );
            }
        }
    }

    // Hyphae trailing across the ridge.
    for _ in 0..45 {
        let mut along = rng.range(-neck.reach, neck.reach);
        let mut across = rng.range(-9.0, 9.0);
        let direction = if rng.f() < 0.5 { 1.0 } else { -1.0 };
        // Heading relative to the axis, so the strand runs roughly along the neck.
        let mut heading: f32 = rng.range(-0.4, 0.4);

        let steps = rng.range(8.0, 22.0) as u32;
        for _ in 0..steps {
            heading += rng.range(-0.3, 0.3);
            along += direction * heading.cos() * 0.7;
            across += heading.sin() * 0.7;

            let fade = 1.0 - smooth(neck.reach - 7.0, neck.reach, along.abs());
            let (x, y) = neck.point(along, across);
            let grey = rng.range(205.0, 245.0);
            img.over(x as i32, y as i32, [grey, grey, grey * 0.96], 0.55 * fade);
        }
    }

    // Sporangia on the join, like the rest of the mass.
    for _ in 0..12 {
        let along = rng.range(-neck.reach + 6.0, neck.reach - 6.0);
        let across = rng.range(-8.0, 8.0);
        let (head_x, head_y) = neck.point(along, across);
        let head_radius = rng.range(0.9, 2.1);

        img.disk(
            head_x + 1.2,
            head_y + 1.5,
            head_radius,
            [50.0, 48.0, 44.0],
            0.35,
        );
        img.disk(head_x, head_y, head_radius, [22.0, 22.0, 26.0], 1.0);
        img.over(
            (head_x + head_radius * 0.35) as i32,
            (head_y - head_radius * 0.45) as i32,
            [120.0, 124.0, 130.0],
            0.9,
        );
    }

    (Rgba::new(size), img)
}
