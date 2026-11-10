//! Colour arithmetic and the one light in the scene: blends, smoothstep, and the Lambert and
//! Blinn terms for the fridge bulb shared by every painter.

/// Linear interpolation from `a` (at `t = 0`) to `b` (at `t = 1`); `t` is not clamped.
pub(crate) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Per-channel blend of two RGB colours, `t` clamped to 0..1 (0 gives `a`, 1 gives `b`).
pub(crate) fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

/// Multiply each channel of `colour` by `factor` (brighten above 1, darken below; no clamp).
pub(crate) fn scale(colour: [f32; 3], factor: f32) -> [f32; 3] {
    [colour[0] * factor, colour[1] * factor, colour[2] * factor]
}

/// Direction to the bulb: top right, in front of the tray (screen space, y down).
/// Roughly unit length.
const LIGHT: [f32; 3] = [0.45, -0.6, 0.66];

/// Lambert term for the unit normal `normal` (not clamped): -1..1, 1 facing the bulb.
pub(crate) fn facing(normal: [f32; 3]) -> f32 {
    normal[0] * LIGHT[0] + normal[1] * LIGHT[1] + normal[2] * LIGHT[2]
}

/// Blinn term for the unit normal `normal` with the viewer straight above (not clamped).
///
/// The half vector is `LIGHT + (0, 0, 1)` normalised, so `normal · half` expands to
/// `(facing(normal) + normal.z) / |LIGHT + (0, 0, 1)|`. Raise it to a power for the highlight.
pub(crate) fn glint(normal: [f32; 3]) -> f32 {
    // |LIGHT + (0, 0, 1)|
    const HALF_LEN: f32 = 1.8216;
    (facing(normal) + normal[2]) / HALF_LEN
}

/// Smoothstep: 0 for `x <= e0`, 1 for `x >= e1`, and a smooth `3t² - 2t³` ramp in between.
/// Assumes `e0 != e1`.
pub(crate) fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
