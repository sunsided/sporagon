//! What is mounted on the wall: the thermostat knob and the bulb.

use super::*;

/// The thermostat knob, over `color` at `(fx, fy)`, lit by `falloff` from the bulb.
pub(super) fn knob(mut color: [f32; 3], fx: f32, fy: f32, falloff: f32) -> [f32; 3] {
    // The thermostat knob, stuck on 7 since 1987. Centred at (52, 52), radius 21, with a
    // 3-pixel shadow ring around it.
    let (knob_x, knob_y) = (fx - 52.0, fy - 52.0);
    let knob_distance = (knob_x * knob_x + knob_y * knob_y).sqrt();
    if knob_distance < 21.0 {
        let angle = knob_y.atan2(knob_x);
        // 18 ridges around the outer ring.
        let grip = if knob_distance > 15.0 && (angle * 18.0).sin() > 0.3 {
            0.8
        } else {
            1.0
        };
        // A shallow dome lit from the top right.
        let dome =
            1.05 - -(0.4 * ((knob_x - 6.0) * 0.6 / 21.0 + (knob_y + 6.0) * 0.8 / 21.0).max(-1.0));
        color = scale([214.0, 210.0, 196.0], dome.clamp(0.5, 1.3) * grip * falloff);

        // The pointer, at 7: a 3.2-pixel bar from 2 to 14 pixels out, at -0.8 rad.
        let (pointer_x, pointer_y) = ((-0.8f32).cos(), (-0.8f32).sin());
        let along = knob_x * pointer_x + knob_y * pointer_y;
        if (knob_x * pointer_y - knob_y * pointer_x).abs() < 1.6 && (2.0..14.0).contains(&along) {
            color = [150.0, 30.0, 24.0];
        }

        if knob_distance > 20.0 {
            color = scale(color, 0.5);
        }
        color = mix(
            color,
            [120.0, 96.0, 50.0],
            fbm(fx / 4.0, fy / 4.0, 71, 2) * 0.35,
        );
    } else if knob_distance < 24.0 {
        color = scale(color, 0.6);
    }
    color
}

/// The bulb, its glow and its rusty mount, over `color` at `(fx, fy)`, `bulb_distance`
/// pixels from the bulb centre.
pub(super) fn bulb(mut color: [f32; 3], fx: f32, fy: f32, bulb_distance: f32) -> [f32; 3] {
    // The bulb in its cage, rusted at the mount: glass inside 15 pixels with a cage bar
    // every 7 pixels, a rusty mount ring out to 22, and a wide glow.
    let glow = (-bulb_distance / 60.0).exp() * 0.5;
    color = mix(color, [255.0, 240.0, 190.0], glow);
    if bulb_distance < 15.0 {
        let core = 1.0 - bulb_distance / 15.0;
        color = mix([255.0, 214.0, 120.0], [255.0, 255.0, 236.0], core * 1.4);
        if ((fx - BULB.0) as i32).rem_euclid(7) == 0 && bulb_distance > 4.0 {
            color = [70.0, 62.0, 50.0];
        }
    } else if bulb_distance < 22.0 {
        color = mix(
            [120.0, 70.0, 36.0],
            [62.0, 40.0, 26.0],
            fbm(fx / 2.0, fy / 2.0, 61, 2),
        );
    }
    color
}
