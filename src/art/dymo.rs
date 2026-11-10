//! Dymo label tape: embossed capitals punched into coloured plastic, stuck slightly crooked on
//! the fridge wall. The font is drawn on a 5x7 grid and shown at least twice its size, so every
//! letter stays crisp after the 256-colour dither.
//!
//! Three units of length appear here:
//! - *font pixels*: one cell of the 5x7 glyph grid;
//! - *screen pixels*: `scale` of them per font pixel;
//! - *supersamples*: `SS` of them per screen pixel, so `scale * SS` per font pixel. The tape is
//!   modelled and shaded at this resolution and averaged down when it is rotated into place.
//!
//! Colours are RGB in 0..255, held as `f32`.

use super::{Rgba, facing, fbm, glint, hash, mix, scale as dim, smooth};

/// Tape colour for the game title.
pub const RED: [f32; 3] = [150.0, 28.0, 30.0];
/// Tape colour for the key help and every label that changes during play.
pub const BLACK: [f32; 3] = [30.0, 30.0, 34.0];
/// Plastic goes white where the embosser stretches it.
const STRETCHED: [f32; 3] = [236.0, 234.0, 224.0];
/// Colour of the specular highlight.
const GLOSS: [f32; 3] = [255.0, 255.0, 250.0];

/// Supersamples per screen pixel, along each axis, of the strip-local image.
const SS: usize = 4;
/// Tape padding left and right of the text, in font pixels.
const PAD_X: usize = 3;
/// Tape padding above and below the text, in font pixels.
const PAD_Y: usize = 2;
/// Shadow offset in screen pixels, down and right: the same direction as the egg tray's
/// shadow on the wall (`fridge.rs`), so labels and tray agree.
const SHADOW: (f32, f32) = (3.0, 4.0);
/// Peak opacity of the shadow, 0..1.
const SHADOW_ALPHA: f32 = 0.45;
/// Room around the tape for the rotation and the shadow, in screen pixels.
const MARGIN: usize = 8;

/// The 5x7 font: per character 7 rows of equal width (1 to 5 font pixels), `#` lit, `.` dark.
#[rustfmt::skip]
const GLYPHS: &[(char, [&str; 7])] = &[
    ('A', [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('B', ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."]),
    ('C', [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."]),
    ('D', ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."]),
    ('E', ["#####", "#....", "#....", "####.", "#....", "#....", "#####"]),
    ('F', ["#####", "#....", "#....", "####.", "#....", "#....", "#...."]),
    ('G', [".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".####"]),
    ('H', ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('I', ["###", ".#.", ".#.", ".#.", ".#.", ".#.", "###"]),
    ('J', ["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."]),
    ('K', ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"]),
    ('L', ["#...", "#...", "#...", "#...", "#...", "#...", "####"]),
    ('M', ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"]),
    ('N', ["#...#", "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#"]),
    ('O', [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('P', ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."]),
    ('Q', [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"]),
    ('R', ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"]),
    ('S', [".####", "#....", "#....", ".###.", "....#", "....#", "####."]),
    ('T', ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."]),
    ('U', ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('V', ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('W', ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "#.#.#", ".#.#."]),
    ('X', ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"]),
    ('Y', ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."]),
    ('Z', ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"]),
    ('0', [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."]),
    ('1', [".#.", "##.", ".#.", ".#.", ".#.", ".#.", "###"]),
    ('2', [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"]),
    ('3', ["####.", "....#", "....#", ".###.", "....#", "....#", "####."]),
    ('4', ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."]),
    ('6', ["..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###."]),
    ('7', ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."]),
    ('8', [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."]),
    ('9', [".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##.."]),
    ('.', [".", ".", ".", ".", ".", ".", "#"]),
    (',', [".", ".", ".", ".", ".", "#", "#"]),
    ('!', ["#", "#", "#", "#", "#", ".", "#"]),
    ('?', [".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#.."]),
    ('-', ["...", "...", "...", "###", "...", "...", "..."]),
    (':', [".", "#", ".", ".", ".", "#", "."]),
    ('\'', ["#", "#", ".", ".", ".", ".", "."]),
];

/// The 7 rows of `c` in the font, matched upper-case; anything the font lacks becomes `?`.
fn glyph(c: char) -> &'static [&'static str; 7] {
    let c = c.to_ascii_uppercase();
    let find = |c| GLYPHS.iter().find(|g| g.0 == c).map(|g| &g.1);
    find(c).or_else(|| find('?')).expect("font has '?'")
}

/// Font-pixel columns of `text`, left to right, each the 7 rows lit or not.
///
/// Neighbouring characters are separated by one blank column, and a space is two blank
/// columns of its own. There is no blank column before the first or after the last character;
/// the tape padding provides that.
fn columns(text: &str) -> Vec<[bool; 7]> {
    let mut cols = Vec::new();

    for (i, c) in text.chars().enumerate() {
        if i > 0 {
            cols.push([false; 7]);
        }

        if c == ' ' {
            cols.extend([[false; 7]; 2]);
            continue;
        }

        let rows = glyph(c);
        for x in 0..rows[0].len() {
            cols.push(std::array::from_fn(|y| rows[y].as_bytes()[x] == b'#'));
        }
    }

    cols
}

#[cfg(test)]
/// Width of the tape for `text` at `scale`, in screen pixels (without margin or tilt).
pub fn width(text: &str, scale: usize) -> usize {
    (columns(text).len() + 2 * PAD_X) * scale
}

/// Box blur of `values` (`w` wide, row-major) with radius `r`, horizontally then vertically.
///
/// Each output is the mean of those of its `2 * r + 1` neighbours that lie inside the image,
/// so the borders are not darkened by zeros outside it. The range of the values is kept.
fn blur(values: &[f32], w: usize, r: usize) -> Vec<f32> {
    let h = values.len() / w;

    let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let mut out = vec![0.0; src.len()];
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0;
                let mut count = 0.0;
                for d in -(r as i32)..=r as i32 {
                    let (sx, sy) = if horizontal {
                        (x as i32 + d, y as i32)
                    } else {
                        (x as i32, y as i32 + d)
                    };
                    if sx >= 0 && sy >= 0 && (sx as usize) < w && (sy as usize) < h {
                        sum += src[sy as usize * w + sx as usize];
                        count += 1.0;
                    }
                }
                out[y * w + x] = sum / count;
            }
        }
        out
    };

    pass(&pass(values, true), false)
}

/// A strip of `tape` with `text` punched into it, letters `scale` screen pixels per font pixel.
///
/// Returns a straight-alpha sprite (RGB 0..255, alpha 0..1) centred on the tape, with its own
/// shadow and `MARGIN` screen pixels of room on every side. `scale` must be at least 1; from 2
/// up the letters survive the dither. `seed` picks the tilt and the grime, so the same
/// arguments always give the same strip.
///
/// The stages, all but the last two in supersamples (`cell = scale * SS` per font pixel):
///
/// 1. **Layout**: the text becomes font-pixel columns, padded by `PAD_X` and `PAD_Y` font
///    pixels of bare tape.
/// 2. **Letter mask**: every lit font pixel fills its cell with 1.0, the rest is 0.0.
/// 3. **Emboss blur**: a box blur of radius `cell / 3` rounds the square pixels into domes
///    (0..1) whose flanks are about two thirds of a font pixel wide, and fuses neighbouring
///    pixels into one stroke.
/// 4. **Tape edge**: signed distance to a rectangle with corners of radius 0.9 font pixels,
///    positive inside. Nothing is drawn outside, and the alpha fades in over the outermost 1.5
///    supersamples to antialias the rim.
/// 5. **Height map**: the tape rises to 0.6 across a bevel 0.6 font pixels wide, and the
///    letters stand up to 0.9 above that.
/// 6. **Shading**: normals from the height gradient; plastic whitened where the letters
///    stretch it, grime, diffuse light, a sharp glint and a gloss streak on the bare tape.
/// 7. **Rotation**: a tilt of up to ±2.5°; every screen pixel averages `SS x SS` point samples
///    of the shaded tape, colour weighted by alpha.
/// 8. **Shadow**: the coverage, blurred, offset by `SHADOW` and at most `SHADOW_ALPHA` opaque,
///    goes under the tape.
pub fn strip(text: &str, tape: [f32; 3], scale: usize, seed: u32) -> Rgba {
    // 1. Layout.
    let cols = columns(text);
    let cell = scale * SS;
    let local_w = (cols.len() + 2 * PAD_X) * cell;
    let local_h = (7 + 2 * PAD_Y) * cell;

    // 2 and 3. Letter mask, rounded into embossed domes.
    let mask = letter_mask(&cols, cell, local_w, local_h);
    let letters = blur(&mask, local_w, cell / 3);

    // 4 to 6. Tape edge, height and shading.
    let local = shade(&letters, tape, seed, cell, local_w, local_h);

    // 7. Stick it on crooked: `hash` is 0..1, so the tilt is -2.5° to 2.5°.
    let tilt = (hash(seed as i32, 7, 91) - 0.5) * 2.0 * 2.5f32.to_radians();
    let (mut sprite, cover) = rotate_down(&local, local_w, local_h, tilt);

    // 8. The shadow.
    cast_shadow(&mut sprite, &cover);
    sprite
}

/// Stage 2 of [`strip`]: 1.0 on every lit font pixel, 0.0 elsewhere, `local_w x local_h`
/// supersamples with `cell` per font pixel. `cols` start `PAD_X` font pixels from the left and
/// `PAD_Y` from the top.
fn letter_mask(cols: &[[bool; 7]], cell: usize, local_w: usize, local_h: usize) -> Vec<f32> {
    let mut mask = vec![0.0; local_w * local_h];

    for (cx, col) in cols.iter().enumerate() {
        for (cy, &is_lit) in col.iter().enumerate() {
            if !is_lit {
                continue;
            }

            let left = (PAD_X + cx) * cell;
            let top = (PAD_Y + cy) * cell;
            for y in 0..cell {
                for x in 0..cell {
                    mask[(top + y) * local_w + left + x] = 1.0;
                }
            }
        }
    }

    mask
}

/// Stage 4 of [`strip`]: signed distance from `(x, y)` to the rim of the tape, in supersamples,
/// positive inside. The tape fills the `local_w x local_h` rectangle with its corners rounded
/// to radius `corner`; this is the usual rounded-box distance with the sign flipped.
fn tape_edge(x: f32, y: f32, local_w: usize, local_h: usize, corner: f32) -> f32 {
    // How far past the straight part of each side; both positive only in a corner square.
    let qx = (x - local_w as f32 / 2.0).abs() - (local_w as f32 / 2.0 - corner);
    let qy = (y - local_h as f32 / 2.0).abs() - (local_h as f32 / 2.0 - corner);

    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let inside = qx.max(qy).min(0.0);
    corner - outside - inside
}

/// Stages 4 to 6 of [`strip`]: the shaded tape, `local_w x local_h` supersamples, as
/// straight-alpha RGBA (RGB 0..255, alpha 0..1). Outside the rim it stays transparent black.
///
/// `letters` is the blurred letter mask (0..1), `cell` the supersamples per font pixel, and
/// `seed` varies the grime and the gloss.
fn shade(
    letters: &[f32],
    tape: [f32; 3],
    seed: u32,
    cell: usize,
    local_w: usize,
    local_h: usize,
) -> Vec<[f32; 4]> {
    let corner = cell as f32 * 0.9;
    let bevel = cell as f32 * 0.6;
    let edge_at =
        |x: usize, y: usize| tape_edge(x as f32 + 0.5, y as f32 + 0.5, local_w, local_h, corner);

    // 0..1.5: the bevel rises to 0.6 over `bevel` supersamples, the letters add up to 0.9.
    let height_at = |x: usize, y: usize| -> f32 {
        let edge_distance = edge_at(x, y);
        0.6 * smooth(0.0, bevel, edge_distance) + 0.9 * letters[y * local_w + x]
    };

    // Turns a height difference across two supersamples into the tilt of the normal. It
    // reduces to 2.6 / scale; 2.6 is tuned by eye.
    let slope_gain = 2.6 / cell as f32 * SS as f32;

    let mut local = vec![[0.0f32; 4]; local_w * local_h];
    for y in 0..local_h {
        for x in 0..local_w {
            let edge_distance = edge_at(x, y);
            if edge_distance <= 0.0 {
                continue;
            }

            // Normal from central differences, clamped at the image border.
            let height = height_at(x, y);
            let dx = height_at((x + 1).min(local_w - 1), y) - height_at(x.saturating_sub(1), y);
            let dy = height_at(x, (y + 1).min(local_h - 1)) - height_at(x, y.saturating_sub(1));
            let slope = [-dx * slope_gain, -dy * slope_gain, 1.0];
            let length = (slope[0] * slope[0] + slope[1] * slope[1] + 1.0).sqrt();
            let normal = slope.map(|v| v / length);

            // Plastic: whitened where the letters stretch it, a bit dirty, lit by the bulb with
            // 0.7 ambient, up to 0.45 direct and up to 0.12 lost to grime.
            let lift = letters[y * local_w + x];
            // Position in screen pixels, so the grime does not change with `SS`.
            let (fx, fy) = (x as f32 / SS as f32, y as f32 / SS as f32);
            let grime = fbm(fx / 9.0, fy / 9.0, seed + 3, 3);
            let whiteness = smooth(0.15, 0.6, lift) * (0.85 + 0.15 * grime);
            let mut col = mix(tape, STRETCHED, whiteness);
            col = dim(col, 0.7 + 0.45 * facing(normal).max(0.0) - 0.12 * grime);

            // Gloss: the tape curls a little, so a soft streak runs along its upper half. It is
            // a Gaussian band centred 0.3 of the way down with a width of 0.12 of the height,
            // 0.12 to 0.22 strong, and only on bare tape.
            let v = y as f32 / local_h as f32;
            let band = (-((v - 0.3) / 0.12).powi(2)).exp();
            let streak = band * (0.12 + 0.1 * fbm(fx / 14.0, 0.0, seed, 2));

            // The glint is sharp (power 30) and duller on the high letter tops.
            let glint_strength = glint(normal).max(0.0).powi(30) * 0.5 * (1.0 - height * 0.3);
            let spec = glint_strength + streak * (1.0 - lift);
            col = mix(col, GLOSS, spec.min(1.0));

            let alpha = smooth(0.0, 1.5, edge_distance);
            local[y * local_w + x] = [col[0], col[1], col[2], alpha];
        }
    }

    local
}

/// Stage 7 of [`strip`]: rotate the shaded tape (`local_w x local_h` supersamples) by `tilt`
/// radians about its centre while averaging it down to screen pixels.
///
/// Each screen pixel takes `SS x SS` point samples at sub-pixel centres, maps them back into
/// the unrotated tape and averages them: colour weighted by alpha, alpha plain. Returns the
/// sprite, sized to the rotated bounding box plus `MARGIN` on every side, and its coverage
/// (alpha, 0..1) per pixel for the shadow.
fn rotate_down(local: &[[f32; 4]], local_w: usize, local_h: usize, tilt: f32) -> (Rgba, Vec<f32>) {
    let (sin, cos) = tilt.sin_cos();
    // The tape's size in screen pixels.
    let (tape_w, tape_h) = (local_w as f32 / SS as f32, local_h as f32 / SS as f32);
    let out_w = (tape_w * cos.abs() + tape_h * sin.abs()).ceil() as usize + 2 * MARGIN;
    let out_h = (tape_w * sin.abs() + tape_h * cos.abs()).ceil() as usize + 2 * MARGIN;

    // The supersample under sprite position `(px, py)` (screen pixels), or transparent.
    let sample = |px: f32, py: f32| -> [f32; 4] {
        // From the sprite centre, rotated back by `-tilt` into tape coordinates.
        let (rx, ry) = (px - out_w as f32 / 2.0, py - out_h as f32 / 2.0);
        let ux = rx * cos + ry * sin + tape_w / 2.0;
        let uy = -rx * sin + ry * cos + tape_h / 2.0;

        let (lx, ly) = ((ux * SS as f32) as i32, (uy * SS as f32) as i32);
        if lx < 0 || ly < 0 || lx >= local_w as i32 || ly >= local_h as i32 {
            return [0.0; 4];
        }
        local[ly as usize * local_w + lx as usize]
    };

    let mut px = vec![[0.0f32; 4]; out_w * out_h];
    let mut cover = vec![0.0f32; out_w * out_h];
    for y in 0..out_h {
        for x in 0..out_w {
            // Premultiplied colour and plain alpha, summed over the supersamples.
            let mut sum = [0.0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let p = sample(
                        x as f32 + (sx as f32 + 0.5) / SS as f32,
                        y as f32 + (sy as f32 + 0.5) / SS as f32,
                    );
                    for i in 0..3 {
                        sum[i] += p[i] * p[3];
                    }
                    sum[3] += p[3];
                }
            }

            let alpha = sum[3] / (SS * SS) as f32;
            cover[y * out_w + x] = alpha;
            if sum[3] > 0.0 {
                px[y * out_w + x] = [sum[0] / sum[3], sum[1] / sum[3], sum[2] / sum[3], alpha];
            }
        }
    }

    (Rgba { w: out_w, px }, cover)
}

/// Stage 8 of [`strip`]: the tape's outline, shifted away from the bulb and softened, under the
/// tape. `cover` is the sprite's alpha (0..1) before the shadow; it is blurred with radius 2,
/// moved by `SHADOW` screen pixels and laid under the sprite as black, at most `SHADOW_ALPHA`
/// opaque.
fn cast_shadow(sprite: &mut Rgba, cover: &[f32]) {
    let w = sprite.w;
    let h = sprite.px.len() / w;
    let shadow = blur(cover, w, 2);

    for y in 0..h {
        for x in 0..w {
            // The tape pixel whose shadow lands here.
            let (sx, sy) = (x as i32 - SHADOW.0 as i32, y as i32 - SHADOW.1 as i32);
            if sx < 0 || sy < 0 {
                continue;
            }
            let shadow_alpha = shadow[sy as usize * w + sx as usize] * SHADOW_ALPHA;

            // Tape over black shadow: the alpha grows, and the straight colour is scaled down
            // so the shadow shows through where the tape is not opaque.
            let p = &mut sprite.px[y * w + x];
            let alpha = p[3] + shadow_alpha * (1.0 - p[3]);
            if alpha > 0.0 {
                for i in 0..3 {
                    p[i] = p[i] * p[3] / alpha;
                }
            }
            p[3] = alpha;
        }
    }
}

/// Composite a strip centred on `(cx, cy)` into a true-colour image `w` screen pixels wide
/// (row-major, RGB 0..255). Whatever falls outside the image is clipped.
pub fn bake(img: &mut [[f32; 3]], w: usize, strip: &Rgba, cx: i32, cy: i32) {
    let h = img.len() / w;
    let strip_h = (strip.px.len() / strip.w) as i32;
    let (x0, y0) = (cx - strip.w as i32 / 2, cy - strip_h / 2);

    for (i, p) in strip.px.iter().enumerate() {
        let (x, y) = (x0 + (i % strip.w) as i32, y0 + (i / strip.w) as i32);
        if p[3] > 0.0 && x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            let o = &mut img[y as usize * w + x as usize];
            *o = mix(*o, [p[0], p[1], p[2]], p[3]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_has_ink_and_strangers_become_question_marks() {
        for (c, rows) in GLYPHS {
            assert!(rows.iter().any(|r| r.contains('#')), "{c:?} is blank");
            assert!(
                rows.iter().all(|r| r.len() == rows[0].len()),
                "{c:?} is ragged"
            );
        }
        assert_eq!(glyph('~'), glyph('?'));
        assert_eq!(glyph('a'), glyph('A'));
    }

    #[test]
    fn the_shadow_falls_away_from_the_bulb() {
        let s = strip("HI", BLACK, 2, 1);
        let h = s.px.len() / s.w;
        let at = |x: usize, y: usize| s.px[y * s.w + x][3];
        // Below and right of the tape: shadow. Above and left, beyond the tape: bare wall.
        assert!(at(s.w - MARGIN + 1, h - MARGIN + 1) > 0.05);
        assert_eq!(at(1, 1), 0.0);
    }
}
