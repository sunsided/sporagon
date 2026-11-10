//! The two moulds, painted as colony sprites at each growth stage.
//!
//! Every sprite is an [`Rgba`] with colour in 0..255 (as `f32`) and alpha in 0..1. Sprite
//! coordinates are pixels with the origin top left and y pointing down; the colony sits on
//! the sprite centre. Light comes from the bulb, top right (see `facing` and `glint`).
//! Growth `g` is a 0..1 fraction of the way from a fresh speck to a full colony.

use super::*;

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
pub(super) fn penicillium(seed: u32, growth: f32) -> Rgba {
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
pub(super) fn rhizopus(seed: u32, growth: f32) -> Rgba {
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
pub(super) fn penicillium_neck(v: (f32, f32), seed: u32, size: usize) -> (Rgba, Rgba) {
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
pub(super) fn rhizopus_neck(v: (f32, f32), seed: u32, size: usize) -> (Rgba, Rgba) {
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
