//! Slime: metaballs shaded as a glossy, translucent film. Used for the mucus puddles under the
//! colonies, the bridges between neighbouring colonies, and the pseudopods and globs that carry
//! a colony to its next cup.
//!
//! All coordinates are pixels with y pointing down, either in screen space or in the local
//! space of a square sprite (top-left at 0, 0). Colours are RGB 0..255 as `f32`, coverage
//! (alpha) is 0..1. Light comes from the fridge bulb, top right, the viewer looks straight down.

use super::{Quantizer, Rgba, facing, fbm, glint, hash, mix, scale, smooth};
use game_toolkit_dosvideo::prelude::Bitmap;
use std::f32::consts::PI;

/// One metaball: a centre and a radius, all in pixels.
#[derive(Clone, Copy, Debug)]
pub struct Ball {
    /// Centre, x (pixels, rightward).
    pub x: f32,
    /// Centre, y (pixels, downward).
    pub y: f32,
    /// Radius (pixels). The surface sits at about this distance from a lone ball's centre.
    pub r: f32,
}

/// Metaball field strength at `(x, y)`; the slime surface is where it reaches 1.
///
/// 1. Each ball contributes `r² / (d² + 0.25)`, exactly 1 at distance `r` (ignoring the
///    `0.25`, which only keeps the centre finite), falling off with the squared distance.
/// 2. The sum is modulated by ±10 % of fractal noise, so the edges are lumpy and the surface
///    carries ripples that catch the light.
///
/// Inside a single ball the value climbs well above 1; [`shade`] treats 1..6 as the height.
fn field(balls: &[Ball], x: f32, y: f32) -> f32 {
    let field_value: f32 = balls
        .iter()
        .map(|ball| ball.r * ball.r / ((x - ball.x).powi(2) + (y - ball.y).powi(2) + 0.25))
        .sum();

    // Ripples: lumpy edges and a surface that catches the light in many small glints.
    // Noise in 0..1 maps to a factor of 0.9..1.1; 4.5 px is the ripple scale.
    field_value * (0.9 + 0.2 * fbm(x / 4.5, y / 4.5, 5, 2))
}

/// Deep and light body colour per side (0 Penicillium, 1 Rhizopus).
const BODY: [([f32; 3], [f32; 3]); 2] = [
    // Penicillium weeps a clear, yellowish exudate.
    ([92.0, 92.0, 54.0], [200.0, 196.0, 140.0]),
    ([96.0, 92.0, 70.0], [204.0, 198.0, 164.0]),
];

/// Pixel offset used for the finite-difference slope of the height field.
const SLOPE_STEP: f32 = 0.8;

/// How steep the slopes look: scales the height gradient into the normal. Larger values make
/// the film look thicker and more domed.
const SLOPE_GAIN: f32 = 9.0;

/// Slime colour and coverage at `(x, y)`, or `None` outside the surface.
///
/// `side` is 0 (Penicillium) or 1 (Rhizopus) and picks the body colour, the inclusions and how
/// opaque the film is. Returns RGB 0..255 (may slightly exceed 255 before the highlight mix)
/// and coverage 0..1.
///
/// 1. Outside the surface (field below 1) there is nothing.
/// 2. The field is turned into a height 0..1 (`sqrt` of a smoothstep over 1..6, so the edge
///    rises steeply like a meniscus and the top is flat).
/// 3. A normal is estimated from forward differences of that height, one step right and one
///    step down.
/// 4. Diffuse (Lambert) and a very sharp Blinn specular from the bulb light the body, mixed
///    between its deep and light colour by height and diffuse.
/// 5. The edge band gets a dark meniscus and a bright refraction line just inside it.
/// 6. Sparse inclusions: carried spores (Rhizopus) or trapped air bubbles (Penicillium).
/// 7. The specular glint is blended in towards near-white, and coverage grows with height so
///    thin edges are more transparent; a glint is always at least as opaque as it is bright.
pub fn shade(balls: &[Ball], side: usize, x: f32, y: f32) -> Option<([f32; 3], f32)> {
    let field_value = field(balls, x, y);
    if field_value < 1.0 {
        return None;
    }

    // Height above the tray, 0 at the surface edge up to 1 deep inside.
    let height_of = |field_value: f32| smooth(1.0, 6.0, field_value).sqrt();
    let height = height_of(field_value);

    // Forward-difference slopes along +x and +y (y down).
    let slope_x = height_of(field(balls, x + SLOPE_STEP, y)) - height;
    let slope_y = height_of(field(balls, x, y + SLOPE_STEP)) - height;

    let (normal_x, normal_y, normal_z) = (-slope_x * SLOPE_GAIN, -slope_y * SLOPE_GAIN, 1.0);
    let length = (normal_x * normal_x + normal_y * normal_y + normal_z * normal_z).sqrt();
    let normal = [normal_x / length, normal_y / length, normal_z / length];

    // Light from the bulb, top right; viewer straight above.
    let diffuse = facing(normal).max(0.0);
    let half = glint(normal);
    // Sharp: only slopes facing the bulb glint, flat tops stay clear.
    let specular = half.max(0.0).powi(70);

    let (deep, light) = BODY[side];
    let body = mix(deep, light, height * 0.4 + diffuse * 0.5);
    let mut colour = scale(body, 0.6 + 0.55 * diffuse);

    // The meniscus at the edge is darker, with a thin bright refraction line inside it.
    if height < 0.12 {
        colour = scale(colour, 0.7);
    } else if height < 0.2 {
        colour = scale(colour, 1.15);
    }

    // Per-pixel random in 0..1, stable across frames (seeded per side).
    let speck = hash(x as i32, y as i32, 900 + side as u32);
    if side == 1 && height > 0.3 && speck < 0.05 {
        colour = [34.0, 32.0, 34.0]; // spores carried along
    } else if side == 0 && height > 0.25 && speck < 0.015 {
        colour = [250.0, 246.0, 220.0]; // trapped air bubble
    }

    // 2.5x so the glint saturates to white before its very narrow peak falls off.
    colour = mix(colour, [255.0, 255.0, 246.0], (specular * 2.5).min(1.0));

    // Penicillium's exudate is clearer than Rhizopus' slime.
    let base_alpha = if side == 0 { 0.42 } else { 0.68 };
    let alpha = base_alpha + 0.3 * smooth(0.0, 0.6, height);
    Some((colour, alpha.max(specular)))
}

/// Inclusive pixel bounding box `(x0, y0, x1, y1)` that contains every point [`shade`] can
/// cover for these balls.
///
/// Each ball is padded to `1.8 r + 2`: neighbouring balls and the +10 % noise push the surface
/// beyond a lone ball's radius. Coordinates are truncated towards zero. An empty slice yields
/// an inverted box (`i32::MAX`, `i32::MAX`, `i32::MIN`, `i32::MIN`).
fn bounds(balls: &[Ball]) -> (i32, i32, i32, i32) {
    balls
        .iter()
        .fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |acc, ball| {
            let pad = ball.r * 1.8 + 2.0;
            (
                acc.0.min((ball.x - pad) as i32),
                acc.1.min((ball.y - pad) as i32),
                acc.2.max((ball.x + pad) as i32),
                acc.3.max((ball.y + pad) as i32),
            )
        })
}

/// Render balls (in sprite coordinates) into a `size` x `size` RGBA sprite.
///
/// Every pixel is shaded at its centre (`+0.5`); the sprite starts fully transparent.
pub fn to_rgba(balls: &[Ball], side: usize, size: usize) -> Rgba {
    let mut img = Rgba::new(size);
    for y in 0..size {
        for x in 0..size {
            if let Some((colour, alpha)) = shade(balls, side, x as f32 + 0.5, y as f32 + 0.5) {
                img.over(x as i32, y as i32, colour, alpha);
            }
        }
    }
    img
}

/// Mix `colour` into the frame pixel `(x, y)` at coverage `alpha`, re-quantising against the
/// palette.
///
/// 1. Off-frame pixels and near-invisible coverage (`<= 0.02`) are skipped.
/// 2. The current palette index is looked up as RGB, mixed linearly towards `colour` by
///    `alpha` (clamped to 0..1 by [`mix`]).
/// 3. The result is mapped back to the nearest (dithered) palette entry.
pub fn blend(
    frame: &mut Bitmap,
    quant: &mut Quantizer,
    x: i32,
    y: i32,
    colour: [f32; 3],
    alpha: f32,
) {
    let outside = x < 0 || y < 0 || x >= frame.width as i32 || y >= frame.height as i32;
    if outside || alpha <= 0.02 {
        return;
    }

    let index = (y * frame.width as i32 + x) as usize;
    let under = quant.pal[frame.data[index] as usize].map(f32::from);
    frame.data[index] = quant.get(mix(under, colour, alpha), x as usize, y as usize);
}

/// Draw a pre-rendered slime sprite with its top-left at screen pixel `(x0, y0)`.
///
/// Pixels with coverage `<= 0.02` are skipped; the rest go through [`blend`], which clips to
/// the frame.
pub fn blit(frame: &mut Bitmap, quant: &mut Quantizer, img: &Rgba, x0: i32, y0: i32) {
    for (i, pixel) in img.px.iter().enumerate() {
        if pixel[3] > 0.02 {
            let (x, y) = ((i % img.w) as i32, (i / img.w) as i32);
            let colour = [pixel[0], pixel[1], pixel[2]];
            blend(frame, quant, x0 + x, y0 + y, colour, pixel[3]);
        }
    }
}

/// Draw slime directly from balls in screen coordinates.
///
/// 1. Find the pixel box the balls can cover ([`bounds`]).
/// 2. Shade every pixel centre in it and [`blend`] the result into the frame.
///
/// Slower than [`blit`] but needed for slime that moves every frame (pseudopods, globs).
pub fn draw(frame: &mut Bitmap, quant: &mut Quantizer, balls: &[Ball], side: usize) {
    if balls.is_empty() {
        return;
    }

    let (x0, y0, x1, y1) = bounds(balls);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if let Some((colour, alpha)) = shade(balls, side, x as f32 + 0.5, y as f32 + 0.5) {
                blend(frame, quant, x, y, colour, alpha);
            }
        }
    }
}

/// Horizontal stretch of a shadow disc: the bulb is low, so shadows fall wider than tall.
const SHADOW_STRETCH: f32 = 1.3;

/// Darken the frame under a soft disc: the shadow of a glob in flight.
///
/// `ball` gives the centre and vertical radius in screen pixels; the disc is
/// [`SHADOW_STRETCH`] times as wide. `strength` (0..1) is the darkening at the centre; it
/// falls off as `1 - d²` towards the rim, where `d` is the 0..1 elliptical distance.
pub fn shadow(frame: &mut Bitmap, quant: &mut Quantizer, ball: Ball, strength: f32) {
    let (top, bottom) = ((ball.y - ball.r) as i32, (ball.y + ball.r) as i32);
    let left = (ball.x - ball.r * SHADOW_STRETCH) as i32;
    let right = (ball.x + ball.r * SHADOW_STRETCH) as i32;

    for y in top..=bottom {
        for x in left..=right {
            let dx = (x as f32 - ball.x) / SHADOW_STRETCH;
            let dy = y as f32 - ball.y;
            let dist_frac = (dx.powi(2) + dy.powi(2)).sqrt() / ball.r;
            if dist_frac < 1.0 {
                let alpha = strength * (1.0 - dist_frac * dist_frac);
                blend(frame, quant, x, y, [20.0, 16.0, 10.0], alpha);
            }
        }
    }
}

/// The mucus puddle a colony sits in, centred in a `size` sprite.
///
/// `growth` (0..1, the colony's stage as a fraction) scales every radius and distance, so the
/// puddle spreads as the colony grows. `seed` picks the shape.
///
/// 1. A central ball of radius `16 * growth`.
/// 2. Seven lobes spaced evenly around it with a random angular jitter of 0..0.8 rad, each
///    at a random distance (14..23) and radius (6..10.5), all scaled by `growth`.
pub fn puddle(seed: u32, growth: f32, size: usize) -> Vec<Ball> {
    let centre = size as f32 / 2.0;
    let mut balls = vec![Ball {
        x: centre,
        y: centre,
        r: 16.0 * growth,
    }];

    for lobe in 0..7 {
        let angle = lobe as f32 / 7.0 * 2.0 * PI + hash(lobe, 0, seed) * 0.8;
        let distance = (14.0 + 9.0 * hash(lobe, 1, seed)) * growth;
        balls.push(Ball {
            x: centre + angle.cos() * distance,
            y: centre + angle.sin() * distance,
            r: (6.0 + 4.5 * hash(lobe, 2, seed)) * growth,
        });
    }
    balls
}

/// A sagging slime bridge across the ridge between two cups, centred in a `size` sprite.
///
/// `offset` is the vector from one cup centre to the other in pixels; the bridge spans it with
/// the sprite centre at its midpoint. `seed` picks the wobble and the side of the drop.
///
/// 1. 17 balls are laid along `offset` at progress 0..1, displaced sideways by a sine wobble
///    plus noise (up to about ±3 px).
/// 2. Radii follow `(2t - 1)²`: 11 px at the cups, 5 px over the ridge, so it sags thin in the
///    middle, plus up to 1.5 px of noise.
/// 3. A drop (radius 4.6) gathers on one flank of the ridge, slightly past the midpoint.
pub fn bridge(offset: (f32, f32), seed: u32, size: usize) -> Vec<Ball> {
    let centre = size as f32 / 2.0;
    let length = (offset.0 * offset.0 + offset.1 * offset.1).sqrt();
    // Unit vector perpendicular to the bridge.
    let (perp_x, perp_y) = (-offset.1 / length, offset.0 / length);

    let segments = 16;
    let mut balls: Vec<Ball> = (0..=segments)
        .map(|k| {
            let progress = k as f32 / segments as f32;
            let wobble = ((progress * 7.0 + seed as f32).sin() + hash(k, 3, seed) - 0.5) * 2.0;
            // 1 at either cup, 0 on the ridge.
            let swell = (2.0 * progress - 1.0).powi(2);
            Ball {
                x: centre + offset.0 * (progress - 0.5) + perp_x * wobble,
                y: centre + offset.1 * (progress - 0.5) + perp_y * wobble,
                r: 5.0 + 6.0 * swell + 1.5 * hash(k, 4, seed),
            }
        })
        .collect();

    // A drop gathering on the ridge.
    let drop_side = if seed.is_multiple_of(2) { 1.0 } else { -1.0 };
    balls.push(Ball {
        x: centre + perp_x * 4.5 * drop_side + offset.0 * 0.06,
        y: centre + perp_y * 4.5 * drop_side + offset.1 * 0.06,
        r: 4.6,
    });
    balls
}

/// Distance (pixels) from the source cup centre where a pseudopod starts.
const CREEP_START: f32 = 8.0;

/// Spacing (pixels) between the balls of a pseudopod's body.
const CREEP_SPACING: f32 = 3.0;

/// A pseudopod creeping from `from` toward `to` (screen pixels).
///
/// `progress` is 0..1: 0 is a stub at the source cup, 1 has the head on `to`. `clock` is a
/// free-running time (frames) driving the peristaltic pulse.
///
/// 1. The head sits on the line from `from` to `to`, `CREEP_START` px out at progress 0 and
///    on `to` at progress 1.
/// 2. Body balls every `CREEP_SPACING` px from `CREEP_START` up to the head, tapering from
///    6 px to 4 px radius, wobbling sideways more the further out they are, and pulsing in
///    radius with a wave that travels along the body.
/// 3. The head ball comes last (callers rely on that), pulsing around 7 px.
///
/// Assumes `from != to`.
pub fn creep(from: (f32, f32), to: (f32, f32), progress: f32, clock: f32) -> Vec<Ball> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt();
    let (dir_x, dir_y) = (dx / length, dy / length);

    let head_dist = CREEP_START + (length - CREEP_START) * progress;
    let segments = ((head_dist - CREEP_START) / CREEP_SPACING) as i32;

    let mut balls: Vec<Ball> = (0..=segments)
        .map(|k| {
            let dist = CREEP_START + k as f32 * CREEP_SPACING;
            // 0 at the root, 1 at the last body ball.
            let frac_along = k as f32 / segments.max(1) as f32;
            let wobble = (k as f32 * 0.7 - clock * 0.25).sin() * 1.6 * frac_along;
            let pulse = 0.9 * (k as f32 * 0.9 - clock * 0.5).sin();
            Ball {
                x: from.0 + dir_x * dist - dir_y * wobble,
                y: from.1 + dir_y * dist + dir_x * wobble,
                r: 6.0 - 2.0 * frac_along + pulse,
            }
        })
        .collect();

    balls.push(Ball {
        x: from.0 + dir_x * head_dist,
        y: from.1 + dir_y * head_dist,
        r: 7.0 + (clock * 0.45).sin(),
    });
    balls
}

/// A glob leaping two cups: it gathers at the edge (`progress` < 0.3), snaps off and arcs over
/// the tray, leaving drops behind. Returns the glob, its shadow on the tray, and the trail.
///
/// `from` and `to` are cup centres in screen pixels, `progress` runs 0..1 over the whole move
/// and `seed` picks the trail. All balls are in screen pixels.
///
/// Gathering (`progress < GATHER`):
/// 1. A short [`creep`] (up to a quarter of the way) with its head swelling from 6 to 10 px.
///    No shadow, no trail.
///
/// Flight (the rest, eased with a smoothstep):
/// 1. The glob's ground point moves linearly from `from` to `to`; it rises on a sine arc up
///    to 16 px (screen up is −y) and swells by up to 35 % at the top.
/// 2. Two tail balls trail behind it along the flight, further back at full speed.
/// 3. For the first 22 % of the flight a thinning thread links it back to `from`.
/// 4. The shadow sits on the ground point, offset down and to the right as the glob rises,
///    and shrinks with height.
/// 5. Up to eight drops are left along the path once the glob has passed them, jittered
///    sideways by up to ±3.5 px.
///
/// At `progress == 1` the first glob ball lies exactly on `to`.
pub fn leap(
    from: (f32, f32),
    to: (f32, f32),
    progress: f32,
    seed: u32,
) -> (Vec<Ball>, Option<Ball>, Vec<Ball>) {
    const GATHER: f32 = 0.3;
    if progress < GATHER {
        let gather_progress = progress / GATHER;
        // `progress * 40` stands in for a clock, so the pulse runs while gathering.
        let mut balls = creep(from, to, gather_progress * 0.25, progress * 40.0);
        if let Some(head) = balls.last_mut() {
            head.r = 6.0 + 4.0 * gather_progress;
        }
        return (balls, None, Vec::new());
    }

    // Flight progress, eased in and out.
    let flight = smooth(0.0, 1.0, (progress - GATHER) / (1.0 - GATHER));
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt();
    let (dir_x, dir_y) = (dx / length, dy / length);

    // 0 at take-off and landing, 1 at the top of the arc.
    let lift = (PI * flight).sin();
    let swell = 1.0 + 0.35 * lift;
    let (ground_x, ground_y) = (from.0 + dx * flight, from.1 + dy * flight);
    let height = lift * 16.0;

    // Stretched along the flight, most at full speed.
    let stretch = 4.0 + 7.0 * lift;
    let mut glob = vec![
        Ball {
            x: ground_x,
            y: ground_y - height,
            r: 8.5 * swell,
        },
        Ball {
            x: ground_x - dir_x * stretch,
            y: ground_y - dir_y * stretch - height * 0.95,
            r: 6.0 * swell,
        },
        Ball {
            x: ground_x - dir_x * stretch * 1.8,
            y: ground_y - dir_y * stretch * 1.8 - height * 0.9,
            r: 3.5 * swell,
        },
    ];

    // The thread back to the old cup, thinning until it snaps at 22 % of the flight.
    let thread = 1.0 - flight / 0.22;
    if thread > 0.0 {
        // One ball every 3 px of the distance flown so far.
        let thread_balls = (flight * length / 3.0) as i32;
        for j in 1..thread_balls {
            let frac = j as f32 / thread_balls as f32;
            glob.push(Ball {
                x: from.0 + dx * flight * frac,
                y: from.1 + dy * flight * frac - height * frac,
                r: 2.6 * thread + 0.6,
            });
        }
    }

    let shadow = Ball {
        x: ground_x + 3.0 * lift,
        y: ground_y + 4.0 * lift,
        r: 9.0 * (1.0 - 0.3 * lift),
    };

    // Drops at 1/9 .. 8/9 of the path, each appearing once the glob has passed it.
    let trail = (1..9)
        .filter(|&j| (j as f32 / 9.0) < flight)
        .map(|j| {
            let frac = j as f32 / 9.0;
            let sideways = (hash(j, 7, seed) - 0.5) * 7.0;
            Ball {
                x: from.0 + dx * frac - dir_y * sideways,
                y: from.1 + dy * frac + dir_x * sideways,
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
