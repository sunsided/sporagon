//! The two moulds, painted as colony sprites at each growth stage, and the necks of mould
//! between neighbouring colonies. Each mould has its own file; this one holds what they
//! share: the wet sheen, the dome lighting and the geometry of a neck.
//!
//! Every sprite is an [`Rgba`] with colour in 0..255 (as `f32`) and alpha in 0..1. Sprite
//! coordinates are pixels with the origin top left and y pointing down; the colony sits on
//! the sprite centre. Light comes from the bulb, top right (see `facing` and `glint`).
//! Growth `g` is a 0..1 fraction of the way from a fresh speck to a full colony.

use super::*;

mod penicillium;
mod rhizopus;

pub(super) use penicillium::{penicillium, penicillium_neck};
pub(super) use rhizopus::{rhizopus, rhizopus_neck};

/// Make a colony look wet: a glossy sheen on the side facing the bulb and a few beads of
/// clear liquid.
///
/// `img` is an already painted `SPR`-square colony sprite, `colony_radius` the colony's
/// nominal radius in sprite pixels around the sprite centre, and `growth` the 0..1 growth
/// stage. Only pixels that are already nearly opaque (alpha > 0.9) get the sheen, so it
/// never brightens the transparent background.
///
/// - Treat the colony as a hemisphere of radius `colony_radius`. For each opaque pixel
///   inside 95 % of it, a tight Blinn highlight from the surface normal gives the sheen,
///   broken up by value noise so it is patchy instead of a perfect spot.
/// - Scatter 2..=7 beads (more on older colonies) inside 80 % of the radius. Each is a faint
///   shadow offset down right, a translucent pale disk, and a one-pixel glint on its upper
///   right, towards the bulb.
fn glisten(img: &mut Rgba, seed: u32, colony_radius: f32, growth: f32) {
    let center = SPR as f32 / 2.0;

    for y in 0..SPR {
        for x in 0..SPR {
            // Offset from the centre in units of the colony radius.
            let (dx, dy) = (
                (x as f32 + 0.5 - center) / colony_radius,
                (y as f32 + 0.5 - center) / colony_radius,
            );
            let radius_frac = (dx * dx + dy * dy).sqrt();
            let pixel = img.px[y * SPR + x];

            if radius_frac < 0.95 && pixel[3] > 0.9 {
                // Height of the unit hemisphere; the flattened x and y (0.8) widen the sheen.
                let height = (1.0 - radius_frac * radius_frac).max(0.0).sqrt();
                let blinn = glint([dx * 0.8, dy * 0.8, height]);
                let patchiness =
                    smooth(0.3, 0.7, vnoise(x as f32 / 3.0, y as f32 / 3.0, seed + 40));
                let sheen = blinn.max(0.0).powi(40) * 0.8 * patchiness;
                img.over(x as i32, y as i32, [250.0, 252.0, 240.0], sheen);
            }
        }
    }

    let mut rng = Rng(0xA5A5_5A5A_1234_4321 ^ (seed as u64 * 31));
    let bead_count = (2.0 + 5.0 * growth) as u32;
    for _ in 0..bead_count {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        // sqrt spreads the beads evenly over the disk instead of crowding the centre.
        let distance = rng.f().sqrt() * 0.8 * colony_radius;
        let (bead_x, bead_y, bead_radius) = (
            center + angle.cos() * distance,
            center + angle.sin() * distance,
            rng.range(0.9, 2.0),
        );

        img.disk(
            bead_x + 0.7,
            bead_y + 0.9,
            bead_radius,
            [20.0, 24.0, 18.0],
            0.3,
        );
        img.disk(bead_x, bead_y, bead_radius, [210.0, 220.0, 205.0], 0.35);
        img.over(
            (bead_x + bead_radius * 0.35) as i32,
            (bead_y - bead_radius * 0.45) as i32,
            [255.0, 255.0, 250.0],
            1.0,
        );
    }
}

/// Dome lighting for a colony: a brightness factor of about 0.72 at the rim, rising towards
/// the side facing the bulb (top right) to at most about 1.12.
///
/// `(dx, dy)` is the pixel's offset from the colony centre in units of the colony radius
/// (y down), and `radius_frac` its length, expected in 0..1; callers clamp it, since values
/// above 1 would flatten the dome to a disk.
///
/// - Model the colony as a unit hemisphere with height `sqrt(1 - r^2)`.
/// - Flatten the normal's slope (0.6) and tilt it up (+0.2) so the shading stays soft: mould
///   is a fuzzy mat, not a glossy ball.
/// - Normalise it, take the Lambert term towards the bulb, and map it onto 0.72..1.12.
fn dome(dx: f32, dy: f32, radius_frac: f32) -> f32 {
    let height = (1.0 - radius_frac * radius_frac).max(0.0).sqrt();
    let (normal_x, normal_y, normal_z) = (dx * 0.6, dy * 0.6, height + 0.2);

    let length = (normal_x * normal_x + normal_y * normal_y + normal_z * normal_z)
        .sqrt()
        .max(1e-4);
    let lambert = facing([normal_x, normal_y, normal_z]) / length;

    0.72 + 0.4 * lambert.max(0.0)
}

/// Where two colonies of one kind touch, the mould grows across the ridge between their cups
/// and they become one mass. A neck is that join: `v` points from one cell centre to the
/// other, the sprite (`size` square) is centred on the midpoint and fades out into both
/// colonies so it covers their facing margins.
///
/// The neck has its own frame: `along` runs from the midpoint towards the second cell
/// (negative towards the first), `across` perpendicular to it, both in sprite pixels.
struct Neck {
    /// Sprite centre, in sprite pixels (the same for x and y).
    center: f32,
    /// Unit vector along the neck, from the first cell to the second.
    axis: (f32, f32),
    /// Unit vector across the neck: `axis` turned a quarter turn.
    normal: (f32, f32),
    /// How far along the axis, either way from the midpoint, the neck reaches before it has
    /// faded out completely, in sprite pixels.
    reach: f32,
}

/// Position of a pixel in the neck: along the axis, across it, the half-width there, and
/// the coverage left after fading into the colonies.
struct NeckPx {
    /// Signed distance along the axis from the midpoint, in sprite pixels.
    along: f32,
    /// Signed distance across the axis, in sprite pixels; its sign tells the two flanks apart.
    across: f32,
    /// Half-width of the neck at this point along it, in sprite pixels.
    half_width: f32,
    /// Coverage 0..1: 1 in the middle, falling to 0 over the last 7 pixels of `reach`.
    fade: f32,
}

impl Neck {
    /// The frame of a neck between two cells `v` apart (in sprite pixels), drawn on a
    /// `size`-square sprite. `v` must not be zero.
    fn new(v: (f32, f32), size: usize) -> Self {
        let distance = (v.0 * v.0 + v.1 * v.1).sqrt();
        let axis = (v.0 / distance, v.1 / distance);

        Self {
            center: size as f32 / 2.0,
            axis,
            normal: (-axis.1, axis.0),
            // Up to the velvet zone of each colony: well past their white margins.
            reach: distance / 2.0 - 9.0,
        }
    }

    /// Where sprite pixel `(x, y)` lies in the neck, and how wide and opaque the neck is
    /// there; `seed` picks the lobes.
    ///
    /// - Project the pixel centre onto the axis and the normal.
    /// - The half-width is a base of 18.5 pixels, wider towards the colonies (quadratic in
    ///   `along`, +3 at `reach`), plus noise lobes of up to about ±4.5 pixels. Each flank
    ///   gets its own lobes, since `across.signum()` feeds the noise.
    /// - Fade out over the last 7 pixels before `reach`.
    fn at(&self, x: usize, y: usize, seed: u32) -> NeckPx {
        let (dx, dy) = (x as f32 + 0.5 - self.center, y as f32 + 0.5 - self.center);
        let along = dx * self.axis.0 + dy * self.axis.1;
        let across = dx * self.normal.0 + dy * self.normal.1;

        // Fat enough that three joined colonies leave no hole between them.
        // Lobed, not a sausage: the edge bulges and dents along its length.
        let lobes = fbm(along / 5.0, across.signum() * 3.0, seed, 3) - 0.5;
        let half_width = 18.5 + 3.0 * (along / self.reach).powi(2) + 9.0 * lobes;
        let fade = 1.0 - smooth(self.reach - 7.0, self.reach, along.abs());

        NeckPx {
            along,
            across,
            half_width,
            fade,
        }
    }

    /// `(x, y)` of a point `along` the axis and `across` it, in sprite pixels.
    fn point(&self, along: f32, across: f32) -> (f32, f32) {
        (
            self.center + self.axis.0 * along + self.normal.0 * across,
            self.center + self.axis.1 * along + self.normal.1 * across,
        )
    }
}
