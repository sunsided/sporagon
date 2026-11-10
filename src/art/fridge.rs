//! The rundown fridge: back wall, egg tray, bulb and grime.

use super::*;

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

pub(super) fn paint_fridge(centers: &[(f32, f32, bool)]) -> Vec<[f32; 3]> {
    let mut rng = Rng(0x5EED_F00D_CAFE_1234);
    let bulb = BULB;
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
    centers: &[(f32, f32, bool)],
    cracks: &[bool],
) -> [f32; 3] {
    let dist_bulb = ((fx - bulb.0).powi(2) + (fy - bulb.1).powi(2)).sqrt();
    let light = 0.62 + 0.7 * (-dist_bulb / 380.0).exp();
    let vignette = 1.0 - 0.35 * (((fx - 320.0) / 340.0).powi(2) + ((fy - 240.0) / 280.0).powi(2));

    // Tray first: it sits in front of everything but the notes. Through a knocked-out cup
    // the fridge wall behind shows.
    let tray = tray_height(fx, fy, centers);
    let through = tray.and_then(|(_, ci)| hole_depth(fx, fy, centers[ci]));
    if let Some((z, ci)) = tray
        && through.is_none()
    {
        let (cx, cy, hole) = centers[ci];
        let h = hex_norm(fx - cx, fy - cy);
        if hole && h < hole_jag(fx, fy) + 0.07 {
            // The snapped-off plastic edge, bright so the hole reads at a glance.
            return scale([250.0, 246.0, 230.0], light.max(1.0) * 1.08);
        }
        let e = 0.8;
        let zx = tray_height(fx + e, fy, centers).map_or(z - 4.0, |t| t.0) - z;
        let zy = tray_height(fx, fy + e, centers).map_or(z - 4.0, |t| t.0) - z;
        let (nx, ny, nz) = (-zx / e, -zy / e, 1.0);
        let l = (nx * nx + ny * ny + nz * nz).sqrt();
        let (nx, ny, nz) = (nx / l, ny / l, nz / l);
        let diffuse = facing([nx, ny, nz]).max(0.0);
        let spec = glint([nx, ny, nz]).max(0.0).powi(40);
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
    let casts = |sx: f32, sy: f32| {
        tray_height(sx, sy, centers)
            .is_some_and(|(_, ci)| hole_depth(sx, sy, centers[ci]).is_none())
    };
    // Down in a hole the tray is close above the wall: soften its shadow edge.
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
        col = scale(col, 0.58);
    }
    if let Some(k) = shade_in_hole {
        col = scale(col, 1.0 - 0.45 * k);
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
    if let Some(d) = through {
        // Seen down through the tray: dimmer, darkest right under the broken edge, with a
        // thin black line where the plastic's cross-section is in its own shadow.
        col = scale(col, 0.6 * (0.5 + 0.5 * smooth(0.0, 0.14, d)));
        if d < 0.025 {
            col = scale(col, 0.35);
        }
    }
    col
}

/// The ragged outline of a knocked-out cup, in hexagonal distance from its centre.
fn hole_jag(x: f32, y: f32) -> f32 {
    0.78 + 0.16 * fbm(x / 5.0, y / 5.0, 77, 2)
}

/// How far inside the opening of a knocked-out cup `(x, y)` lies, if it does.
fn hole_depth(x: f32, y: f32, (cx, cy, hole): (f32, f32, bool)) -> Option<f32> {
    let d = hole_jag(x, y) - hex_norm(x - cx, y - cy);
    (hole && d > 0.0).then_some(d)
}
