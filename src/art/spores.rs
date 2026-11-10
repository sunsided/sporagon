//! The two moulds, painted as colony sprites at each growth stage.

use super::*;

/// Make a colony look wet: a glossy sheen on the side facing the bulb and a few beads of
/// clear liquid.
fn glisten(img: &mut Rgba, seed: u32, rc: f32, g: f32) {
    let c = SPR as f32 / 2.0;
    for y in 0..SPR {
        for x in 0..SPR {
            let (dx, dy) = ((x as f32 + 0.5 - c) / rc, (y as f32 + 0.5 - c) / rc);
            let nr = (dx * dx + dy * dy).sqrt();
            let p = img.px[y * SPR + x];
            if nr < 0.95 && p[3] > 0.9 {
                let h = (1.0 - nr * nr).max(0.0).sqrt();
                let half = (dx * 0.8 * 0.45 - dy * 0.8 * 0.6 + h * 1.66) / 1.79;
                let sheen = half.max(0.0).powi(40)
                    * 0.8
                    * smooth(0.3, 0.7, vnoise(x as f32 / 3.0, y as f32 / 3.0, seed + 40));
                img.over(x as i32, y as i32, [250.0, 252.0, 240.0], sheen);
            }
        }
    }
    let mut rng = Rng(0xA5A5_5A5A_1234_4321 ^ (seed as u64 * 31));
    for _ in 0..(2.0 + 5.0 * g) as u32 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let d = rng.f().sqrt() * 0.8 * rc;
        let (px, py, rr) = (c + a.cos() * d, c + a.sin() * d, rng.range(0.9, 2.0));
        img.disk(px + 0.7, py + 0.9, rr, [20.0, 24.0, 18.0], 0.3);
        img.disk(px, py, rr, [210.0, 220.0, 205.0], 0.35);
        img.over(
            (px + rr * 0.35) as i32,
            (py - rr * 0.45) as i32,
            [255.0, 255.0, 250.0],
            1.0,
        );
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
pub(super) fn penicillium(seed: u32, g: f32) -> Rgba {
    let mut img = Rgba::new(SPR);
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
    glisten(&mut img, seed, rc, g);
    img
}

/// Rhizopus, black bread mould: a tangle of white cottony hyphae dotted with sporangia that
/// start white and turn black as they ripen.
pub(super) fn rhizopus(seed: u32, g: f32) -> Rgba {
    let mut img = Rgba::new(SPR);
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
    glisten(&mut img, seed, rc, g);
    img
}

/// Where two colonies of one kind touch, the mould grows across the ridge between their cups
/// and they become one mass. A neck is that join: `v` points from one cell centre to the
/// other, the sprite (`size` square) is centred on the midpoint and fades out into both
/// colonies so it covers their facing margins.
struct Neck {
    c: f32,
    u: (f32, f32),
    p: (f32, f32),
    reach: f32,
}

/// Position of a pixel in the neck: along the axis, across it, the half-width there, and
/// the coverage left after fading into the colonies.
struct NeckPx {
    t: f32,
    l: f32,
    w: f32,
    fade: f32,
}

impl Neck {
    fn new(v: (f32, f32), size: usize) -> Self {
        let len = (v.0 * v.0 + v.1 * v.1).sqrt();
        let u = (v.0 / len, v.1 / len);
        Self {
            c: size as f32 / 2.0,
            u,
            p: (-u.1, u.0),
            // Up to the velvet zone of each colony: well past their white margins.
            reach: len / 2.0 - 9.0,
        }
    }

    fn at(&self, x: usize, y: usize, seed: u32) -> NeckPx {
        let (dx, dy) = (x as f32 + 0.5 - self.c, y as f32 + 0.5 - self.c);
        let t = dx * self.u.0 + dy * self.u.1;
        let l = dx * self.p.0 + dy * self.p.1;
        // Fat enough that three joined colonies leave no hole between them.
        // Lobed, not a sausage: the edge bulges and dents along its length.
        let lobes = fbm(t / 5.0, l.signum() * 3.0, seed, 3) - 0.5;
        let w = 18.5 + 3.0 * (t / self.reach).powi(2) + 9.0 * lobes;
        let fade = 1.0 - smooth(self.reach - 7.0, self.reach, t.abs());
        NeckPx { t, l, w, fade }
    }

    /// `(x, y)` of a point `t` along and `l` across, in sprite pixels.
    fn point(&self, t: f32, l: f32) -> (f32, f32) {
        (
            self.c + self.u.0 * t + self.p.0 * l,
            self.c + self.u.1 * t + self.p.1 * l,
        )
    }
}

/// Penicillium's join as `(rim, core)`: draw every rim first and every core after, so the
/// cores cover the margins where joins cross inside a mass.
pub(super) fn penicillium_neck(v: (f32, f32), seed: u32, size: usize) -> (Rgba, Rgba) {
    let mut rim_img = Rgba::new(size);
    let mut img = Rgba::new(size);
    let neck = Neck::new(v, size);
    for y in 0..size {
        for x in 0..size {
            let n = neck.at(x, y, seed);
            if n.fade <= 0.0 {
                continue;
            }
            // Ragged edge, pixel by pixel.
            let nr =
                n.l.abs() / n.w + 0.16 * (fbm(x as f32 / 2.5, y as f32 / 2.5, seed + 21, 2) - 0.5);
            let fine = fbm(x as f32 / 1.7, y as f32 / 1.7, seed + 11, 2);
            if nr < 1.0 {
                let velvet = [56.0, 100.0, 88.0];
                let velvet2 = [88.0, 128.0, 106.0];
                let rim = [158.0, 194.0, 160.0];
                let margin = [238.0, 240.0, 228.0];
                let layer = if nr < 0.78 { &mut img } else { &mut rim_img };
                // Powdery velvet in patches, no pattern.
                let patch = fbm(x as f32 / 5.0, y as f32 / 5.0, seed + 2, 3);
                let mut col = mix(velvet, velvet2, smooth(0.3, 0.7, patch));
                // The white margin is ragged and wider where the edge bulges.
                let m0 = 0.66 + 0.12 * fbm(n.t / 4.0, n.l.signum(), seed + 9, 2);
                if nr > m0 {
                    col = mix(col, rim, smooth(m0, m0 + 0.12, nr));
                    col = mix(col, margin, smooth(m0 + 0.12, 1.0, nr));
                }
                let dust = smooth(0.5, 0.8, fbm(x as f32 / 3.5, y as f32 / 3.5, seed + 13, 3));
                col = mix(
                    col,
                    [150.0, 176.0, 158.0],
                    dust * 0.55 * (1.0 - smooth(0.6, 0.85, nr)),
                );
                // Fine wrinkles where the mat buckled as it grew over the ridge.
                let wrinkle = (fbm(x as f32 / 6.0, y as f32 / 6.0, seed + 17, 3) - 0.5).abs();
                let s = n.l.signum() * nr.min(1.0);
                let k = dome(neck.p.0 * s, neck.p.1 * s, nr.min(1.0))
                    * (0.84 + 0.32 * fine)
                    * (0.82 + 0.18 * smooth(0.0, 0.06, wrinkle));
                col = scale(col, k);
                let h = hash(x as i32, y as i32, seed + 3);
                if nr < 0.8 && h > 0.93 {
                    col = scale(col, 1.25);
                } else if nr < 0.8 && h < 0.05 {
                    col = scale(col, 0.7);
                }
                layer.over(x as i32, y as i32, col, n.fade);
            } else if nr < 1.25 {
                let fuzz = fbm(x as f32 / 1.3, y as f32 / 1.3, seed + 7, 2);
                let a = (1.0 - (nr - 1.0) / 0.25).powf(1.5) * smooth(0.3, 0.7, fuzz);
                rim_img.over(x as i32, y as i32, [222.0, 228.0, 216.0], a * 0.85 * n.fade);
            }
        }
    }
    (rim_img, img)
}

/// Rhizopus' join as `(rim, core)`; cotton has no margin to hide, so the rim is empty.
pub(super) fn rhizopus_neck(v: (f32, f32), seed: u32, size: usize) -> (Rgba, Rgba) {
    let mut img = Rgba::new(size);
    let neck = Neck::new(v, size);
    let mut rng = Rng(0x7F4A_7C15_D1B5_4A32 ^ (seed as u64 * 2_654_435_761));
    for y in 0..size {
        for x in 0..size {
            let n = neck.at(x, y, seed);
            let nr = n.l.abs() / n.w;
            let fluff = fbm(x as f32 / 2.3, y as f32 / 2.3, seed + 5, 3);
            let dens = (1.0 - nr * nr).max(0.0) * (0.45 + 0.9 * fluff);
            if dens > 0.0 && n.fade > 0.0 {
                let grey = lerp(150.0, 246.0, fluff.powf(0.8));
                let s = n.l.signum() * nr.min(1.0);
                let col = scale(
                    [grey, grey * 0.99, grey * 0.94],
                    dome(neck.p.0 * s, neck.p.1 * s, nr.min(1.0)),
                );
                img.over(x as i32, y as i32, col, smooth(0.08, 0.4, dens) * n.fade);
            }
        }
    }
    // Hyphae trailing across the ridge.
    for _ in 0..45 {
        let mut t = rng.range(-neck.reach, neck.reach);
        let mut l = rng.range(-9.0, 9.0);
        let dir = if rng.f() < 0.5 { 1.0 } else { -1.0 };
        let mut a: f32 = rng.range(-0.4, 0.4);
        for _ in 0..rng.range(8.0, 22.0) as u32 {
            a += rng.range(-0.3, 0.3);
            t += dir * a.cos() * 0.7;
            l += a.sin() * 0.7;
            let fade = 1.0 - smooth(neck.reach - 7.0, neck.reach, t.abs());
            let (x, y) = neck.point(t, l);
            let g = rng.range(205.0, 245.0);
            img.over(x as i32, y as i32, [g, g, g * 0.96], 0.55 * fade);
        }
    }
    // Sporangia on the join, like the rest of the mass.
    for _ in 0..12 {
        let t = rng.range(-neck.reach + 6.0, neck.reach - 6.0);
        let l = rng.range(-8.0, 8.0);
        let (px, py) = neck.point(t, l);
        let rr = rng.range(0.9, 2.1);
        img.disk(px + 1.2, py + 1.5, rr, [50.0, 48.0, 44.0], 0.35);
        img.disk(px, py, rr, [22.0, 22.0, 26.0], 1.0);
        img.over(
            (px + rr * 0.35) as i32,
            (py - rr * 0.45) as i32,
            [120.0, 124.0, 130.0],
            0.9,
        );
    }
    (Rgba::new(size), img)
}
