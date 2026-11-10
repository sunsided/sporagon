//! Slime: metaballs shaded as a glossy, translucent film. Used for the mucus puddles under the
//! colonies, the bridges between neighbouring colonies, and the pseudopods and globs that carry
//! a colony to its next cup.

use super::{Quantizer, Rgba, fbm, hash, mix, scale, smooth};
use game_toolkit_dosvideo::prelude::Bitmap;
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

fn field(balls: &[Ball], x: f32, y: f32) -> f32 {
    let f: f32 = balls
        .iter()
        .map(|b| b.r * b.r / ((x - b.x).powi(2) + (y - b.y).powi(2) + 0.25))
        .sum();
    // Ripples: lumpy edges and a surface that catches the light in many small glints.
    f * (0.9 + 0.2 * fbm(x / 4.5, y / 4.5, 5, 2))
}

/// Deep and light body colour per side (0 Penicillium, 1 Rhizopus).
const BODY: [([f32; 3], [f32; 3]); 2] = [
    // Penicillium weeps a clear, yellowish exudate.
    ([92.0, 92.0, 54.0], [200.0, 196.0, 140.0]),
    ([96.0, 92.0, 70.0], [204.0, 198.0, 164.0]),
];

/// Slime colour and coverage at `(x, y)`, or `None` outside the surface.
pub fn shade(balls: &[Ball], side: usize, x: f32, y: f32) -> Option<([f32; 3], f32)> {
    let f = field(balls, x, y);
    if f < 1.0 {
        return None;
    }
    let z = |f: f32| smooth(1.0, 6.0, f).sqrt();
    let h = z(f);
    let zx = z(field(balls, x + 0.8, y)) - h;
    let zy = z(field(balls, x, y + 0.8)) - h;
    let (nx, ny, nz) = (-zx * 9.0, -zy * 9.0, 1.0);
    let l = (nx * nx + ny * ny + nz * nz).sqrt();
    let (nx, ny, nz) = (nx / l, ny / l, nz / l);
    // Light from the bulb, top right; viewer straight above.
    let diffuse = (nx * 0.45 - ny * 0.6 + nz * 0.66).max(0.0);
    let half = (nx * 0.45 - ny * 0.6 + nz * 1.66) / 1.82;
    // Sharp: only slopes facing the bulb glint, flat tops stay clear.
    let spec = half.max(0.0).powi(70);
    let (deep, light) = BODY[side];
    let mut col = scale(
        mix(deep, light, h * 0.4 + diffuse * 0.5),
        0.6 + 0.55 * diffuse,
    );
    // The meniscus at the edge is darker, with a thin bright refraction line inside it.
    if h < 0.12 {
        col = scale(col, 0.7);
    } else if h < 0.2 {
        col = scale(col, 1.15);
    }
    let speck = hash(x as i32, y as i32, 900 + side as u32);
    if side == 1 && h > 0.3 && speck < 0.05 {
        col = [34.0, 32.0, 34.0]; // spores carried along
    } else if side == 0 && h > 0.25 && speck < 0.015 {
        col = [250.0, 246.0, 220.0]; // trapped air bubble
    }
    col = mix(col, [255.0, 255.0, 246.0], (spec * 2.5).min(1.0));
    let a = if side == 0 { 0.42 } else { 0.68 } + 0.3 * smooth(0.0, 0.6, h);
    Some((col, a.max(spec)))
}

fn bounds(balls: &[Ball]) -> (i32, i32, i32, i32) {
    balls
        .iter()
        .fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |b, s| {
            let p = s.r * 1.8 + 2.0;
            (
                b.0.min((s.x - p) as i32),
                b.1.min((s.y - p) as i32),
                b.2.max((s.x + p) as i32),
                b.3.max((s.y + p) as i32),
            )
        })
}

/// Render balls (in sprite coordinates) into an RGBA sprite.
pub fn to_rgba(balls: &[Ball], side: usize, size: usize) -> Rgba {
    let mut img = Rgba::new(size);
    for y in 0..size {
        for x in 0..size {
            if let Some((c, a)) = shade(balls, side, x as f32 + 0.5, y as f32 + 0.5) {
                img.over(x as i32, y as i32, c, a);
            }
        }
    }
    img
}

/// Mix `c` into the frame at coverage `a`, re-quantising against the palette.
pub fn blend(frame: &mut Bitmap, q: &mut Quantizer, x: i32, y: i32, c: [f32; 3], a: f32) {
    if x < 0 || y < 0 || x >= frame.width as i32 || y >= frame.height as i32 || a <= 0.02 {
        return;
    }
    let i = (y * frame.width as i32 + x) as usize;
    let under = q.pal[frame.data[i] as usize].map(f32::from);
    frame.data[i] = q.get(mix(under, c, a), x as usize, y as usize);
}

/// Draw a pre-rendered slime sprite with its top-left at `(x0, y0)`.
pub fn blit(frame: &mut Bitmap, q: &mut Quantizer, img: &Rgba, x0: i32, y0: i32) {
    for (i, p) in img.px.iter().enumerate() {
        if p[3] > 0.02 {
            let (x, y) = ((i % img.w) as i32, (i / img.w) as i32);
            blend(frame, q, x0 + x, y0 + y, [p[0], p[1], p[2]], p[3]);
        }
    }
}

/// Draw slime directly from balls in screen coordinates.
pub fn draw(frame: &mut Bitmap, q: &mut Quantizer, balls: &[Ball], side: usize) {
    if balls.is_empty() {
        return;
    }
    let (x0, y0, x1, y1) = bounds(balls);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if let Some((c, a)) = shade(balls, side, x as f32 + 0.5, y as f32 + 0.5) {
                blend(frame, q, x, y, c, a);
            }
        }
    }
}

/// Darken the frame under a soft disc: the shadow of a glob in flight.
pub fn shadow(frame: &mut Bitmap, q: &mut Quantizer, b: Ball, strength: f32) {
    for y in (b.y - b.r) as i32..=(b.y + b.r) as i32 {
        for x in (b.x - b.r * 1.3) as i32..=(b.x + b.r * 1.3) as i32 {
            let d = (((x as f32 - b.x) / 1.3).powi(2) + (y as f32 - b.y).powi(2)).sqrt() / b.r;
            if d < 1.0 {
                blend(frame, q, x, y, [20.0, 16.0, 10.0], strength * (1.0 - d * d));
            }
        }
    }
}

/// The mucus puddle a colony sits in, `g` its growth, centred in an `size` sprite.
pub fn puddle(seed: u32, g: f32, size: usize) -> Vec<Ball> {
    let c = size as f32 / 2.0;
    let mut balls = vec![Ball {
        x: c,
        y: c,
        r: 16.0 * g,
    }];
    for k in 0..7 {
        let a = k as f32 / 7.0 * 2.0 * PI + hash(k, 0, seed) * 0.8;
        let d = (14.0 + 9.0 * hash(k, 1, seed)) * g;
        balls.push(Ball {
            x: c + a.cos() * d,
            y: c + a.sin() * d,
            r: (6.0 + 4.5 * hash(k, 2, seed)) * g,
        });
    }
    balls
}

/// A sagging slime bridge across the ridge between two cups, `v` from one centre to the
/// other, centred in a `size` sprite.
pub fn bridge(v: (f32, f32), seed: u32, size: usize) -> Vec<Ball> {
    let c = size as f32 / 2.0;
    let len = (v.0 * v.0 + v.1 * v.1).sqrt();
    let (px, py) = (-v.1 / len, v.0 / len);
    let n = 16;
    let mut balls: Vec<Ball> = (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            let wob = ((t * 7.0 + seed as f32).sin() + hash(k, 3, seed) - 0.5) * 2.0;
            let neck = (2.0 * t - 1.0).powi(2);
            Ball {
                x: c + v.0 * (t - 0.5) + px * wob,
                y: c + v.1 * (t - 0.5) + py * wob,
                r: 5.0 + 6.0 * neck + 1.5 * hash(k, 4, seed),
            }
        })
        .collect();
    // A drop gathering on the ridge.
    let side = if seed.is_multiple_of(2) { 1.0 } else { -1.0 };
    balls.push(Ball {
        x: c + px * 4.5 * side + v.0 * 0.06,
        y: c + py * 4.5 * side + v.1 * 0.06,
        r: 4.6,
    });
    balls
}

/// A pseudopod creeping from `from` toward `to`; `p` is progress 0..1, `t` a clock for the
/// peristaltic pulse.
pub fn creep(from: (f32, f32), to: (f32, f32), p: f32, t: f32) -> Vec<Ball> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = (dx / len, dy / len);
    let head = 8.0 + (len - 8.0) * p;
    let n = ((head - 8.0) / 3.0) as i32;
    let mut balls: Vec<Ball> = (0..=n)
        .map(|k| {
            let d = 8.0 + k as f32 * 3.0;
            let f = k as f32 / n.max(1) as f32;
            let wob = (k as f32 * 0.7 - t * 0.25).sin() * 1.6 * f;
            Ball {
                x: from.0 + ux * d - uy * wob,
                y: from.1 + uy * d + ux * wob,
                r: 6.0 - 2.0 * f + 0.9 * (k as f32 * 0.9 - t * 0.5).sin(),
            }
        })
        .collect();
    balls.push(Ball {
        x: from.0 + ux * head,
        y: from.1 + uy * head,
        r: 7.0 + (t * 0.45).sin(),
    });
    balls
}

/// A glob leaping two cups: it gathers at the edge (`s` < 0.3), snaps off and arcs over the
/// tray, leaving drops behind. Returns the glob, its shadow on the tray, and the trail.
pub fn leap(
    from: (f32, f32),
    to: (f32, f32),
    s: f32,
    seed: u32,
) -> (Vec<Ball>, Option<Ball>, Vec<Ball>) {
    const GATHER: f32 = 0.3;
    if s < GATHER {
        let p = s / GATHER;
        let mut b = creep(from, to, p * 0.25, s * 40.0);
        if let Some(head) = b.last_mut() {
            head.r = 6.0 + 4.0 * p;
        }
        return (b, None, Vec::new());
    }
    let u = smooth(0.0, 1.0, (s - GATHER) / (1.0 - GATHER));
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = (dx / len, dy / len);
    let lift = (PI * u).sin();
    let k = 1.0 + 0.35 * lift;
    let (gx, gy) = (from.0 + dx * u, from.1 + dy * u);
    let up = lift * 16.0;
    // Stretched along the flight, most at full speed.
    let stretch = 4.0 + 7.0 * lift;
    let mut glob = vec![
        Ball {
            x: gx,
            y: gy - up,
            r: 8.5 * k,
        },
        Ball {
            x: gx - ux * stretch,
            y: gy - uy * stretch - up * 0.95,
            r: 6.0 * k,
        },
        Ball {
            x: gx - ux * stretch * 1.8,
            y: gy - uy * stretch * 1.8 - up * 0.9,
            r: 3.5 * k,
        },
    ];
    // The thread back to the old cup, thinning until it snaps.
    let thread = 1.0 - u / 0.22;
    if thread > 0.0 {
        let n = (u * len / 3.0) as i32;
        for j in 1..n {
            let f = j as f32 / n as f32;
            glob.push(Ball {
                x: from.0 + dx * u * f,
                y: from.1 + dy * u * f - up * f,
                r: 2.6 * thread + 0.6,
            });
        }
    }
    let shadow = Ball {
        x: gx + 3.0 * lift,
        y: gy + 4.0 * lift,
        r: 9.0 * (1.0 - 0.3 * lift),
    };
    let trail = (1..9)
        .filter(|&j| (j as f32 / 9.0) < u)
        .map(|j| {
            let f = j as f32 / 9.0;
            let o = (hash(j, 7, seed) - 0.5) * 7.0;
            Ball {
                x: from.0 + dx * f - uy * o,
                y: from.1 + dy * f + ux * o,
                r: 1.4 + 1.4 * hash(j, 8, seed),
            }
        })
        .collect();
    (glob, Some(shadow), trail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ball_covers_its_radius() {
        let b = [Ball {
            x: 10.0,
            y: 10.0,
            r: 5.0,
        }];
        assert!(shade(&b, 0, 10.0, 10.0).is_some());
        assert!(shade(&b, 0, 12.0, 10.0).is_some());
        assert!(shade(&b, 0, 16.0, 10.0).is_none());
    }

    #[test]
    fn creep_reaches_the_target() {
        let b = creep((0.0, 0.0), (52.0, 0.0), 1.0, 0.0);
        let head = b.last().unwrap();
        assert!((head.x - 52.0).abs() < 0.01 && head.y.abs() < 0.01);
        // Continuous: no gap wider than the balls can bridge.
        assert!(
            b.windows(2)
                .all(|w| (w[1].x - w[0].x).abs() < w[0].r + w[1].r)
        );
    }

    #[test]
    fn leap_lands() {
        let (glob, shadow, trail) = leap((0.0, 0.0), (104.0, 0.0), 1.0, 1);
        assert!((glob[0].x - 104.0).abs() < 0.01 && glob[0].y.abs() < 0.01);
        assert!(shadow.is_some());
        assert_eq!(trail.len(), 8);
    }
}
