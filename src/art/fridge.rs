//! The rundown fridge: back wall, egg tray, bulb and grime.
//!
//! Everything here is in screen pixels on the `W` x `H` (640 x 480) background, origin top
//! left, y down. Colours are RGB 0..255 as `f32`, unclamped; quantising to the palette
//! happens later. The tray is described by `centers`: one `(x, y, hole)` per board cell,
//! where `hole` marks a knocked-out cup through which the back wall shows.

use super::*;

mod drops;
mod fixtures;
mod tray;
mod wall;

use drops::{Drop, apply_drop, droplets};

/// Everything scattered once, up front, and read by every pixel.
struct Scatter {
    drips: Vec<wall::Drip>,
    wall_drops: Vec<Drop>,
    tray_drops: Vec<Drop>,
    /// `W * H` mask of the cracks around the knocked-out cups.
    cracks: Vec<bool>,
}

/// The bulb's light at one pixel.
struct Light {
    /// Distance from the bulb centre, in pixels.
    bulb_distance: f32,
    /// Brightness falling off with distance: 0.62 far away, up to 1.32 at the bulb.
    falloff: f32,
    /// Elliptical vignette: 1 at the screen centre (320, 240), about 0.7 mid-edge and 0.43
    /// in the corners. Each layer clamps it from below.
    vignette: f32,
}

impl Light {
    fn at(fx: f32, fy: f32) -> Self {
        let bulb_distance = ((fx - BULB.0).powi(2) + (fy - BULB.1).powi(2)).sqrt();
        Light {
            bulb_distance,
            falloff: 0.62 + 0.7 * (-bulb_distance / 380.0).exp(),
            vignette: 1.0
                - 0.35 * (((fx - 320.0) / 340.0).powi(2) + ((fy - 240.0) / 280.0).powi(2)),
        }
    }
}

/// Paint the whole fridge background: one RGB colour (0..255 as `f32`) per screen pixel,
/// row by row, `W * H` entries.
///
/// `centers` lists the board cells as `(x, y, hole)` in screen pixels. The result is fully
/// deterministic: the RNG has a fixed seed, and everything is scattered in this order: 11
/// drips, 160 wall drops, 140 drops over the tray's bounding box, then the cracks.
pub(super) fn paint_fridge(centers: &[(f32, f32, bool)]) -> Vec<[f32; 3]> {
    let mut rng = Rng(0x5EED_F00D_CAFE_1234);
    let drips = wall::scatter_drips(&mut rng);
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
    let cracks = tray::scatter_cracks(&mut rng, centers);
    let scatter = Scatter {
        drips,
        wall_drops,
        tray_drops,
        cracks,
    };

    let mut out = vec![[0.0; 3]; W * H];
    for y in 0..H {
        for x in 0..W {
            out[y * W + x] = fridge_pixel(x as f32, y as f32, centers, &scatter);
        }
    }
    out
}

/// The colour of one background pixel at screen position `(fx, fy)`, RGB 0..255 as `f32`.
/// `(fx, fy)` must be on screen, since it indexes the crack mask.
///
/// The tray sits in front of the wall, except where a knocked-out cup shows the wall
/// behind; the wall is painted in layers from the plastic up.
fn fridge_pixel(fx: f32, fy: f32, centers: &[(f32, f32, bool)], scatter: &Scatter) -> [f32; 3] {
    let light = Light::at(fx, fy);

    let on_tray = tray::tray_height(fx, fy, centers);
    let through = on_tray.and_then(|(_, cell)| tray::hole_depth(fx, fy, centers[cell]));
    if let Some(tray_point) = on_tray
        && through.is_none()
    {
        return tray::shade(fx, fy, tray_point, &light, centers, scatter);
    }

    let mut color = wall::paint(fx, fy, &scatter.drips);
    color = scale(color, light.falloff * light.vignette.max(0.35));
    color = tray::cast_shadow(color, fx, fy, centers, through);
    color = fixtures::knob(color, fx, fy, light.falloff);
    color = fixtures::bulb(color, fx, fy, light.bulb_distance);

    // Condensation on the wall.
    if let Some(dropped) = scatter
        .wall_drops
        .iter()
        .find_map(|d| apply_drop(color, d, fx, fy))
    {
        color = dropped;
    }

    if let Some(depth) = through {
        color = tray::seen_through(color, depth);
    }
    color
}
