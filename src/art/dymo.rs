//! Dymo label tape: embossed capitals punched into coloured plastic, stuck slightly crooked on
//! the fridge wall. The font is drawn on a 5x7 grid and shown at least twice its size, so every
//! letter stays crisp after the 256-colour dither.

use super::{Rgba, facing, fbm, glint, hash, mix, scale as dim, smooth};

pub const RED: [f32; 3] = [150.0, 28.0, 30.0];
pub const GREEN: [f32; 3] = [30.0, 96.0, 70.0];
pub const GREY: [f32; 3] = [78.0, 80.0, 84.0];
pub const BLACK: [f32; 3] = [30.0, 30.0, 34.0];
/// Plastic goes white where the embosser stretches it.
const STRETCHED: [f32; 3] = [236.0, 234.0, 224.0];

/// Supersampling of the strip-local image.
const SS: usize = 4;
/// Tape padding around the text, in font pixels.
const PAD_X: usize = 3;
const PAD_Y: usize = 2;
/// Shadow offset in screen pixels; the bulb is up and to the left of the wall column.
const SHADOW: (f32, f32) = (3.0, 4.0);
/// Room around the tape for the rotation and the shadow, in screen pixels.
const MARGIN: usize = 8;

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

fn glyph(c: char) -> &'static [&'static str; 7] {
    let c = c.to_ascii_uppercase();
    let find = |c| GLYPHS.iter().find(|g| g.0 == c).map(|g| &g.1);
    find(c).or_else(|| find('?')).expect("font has '?'")
}

/// Font-pixel columns of `text`, each the 7 rows lit or not.
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
        let g = glyph(c);
        for x in 0..g[0].len() {
            cols.push(std::array::from_fn(|y| g[y].as_bytes()[x] == b'#'));
        }
    }
    cols
}

#[cfg(test)]
/// Width of the tape for `text` at `scale`, in screen pixels (without margin or tilt).
pub fn width(text: &str, scale: usize) -> usize {
    (columns(text).len() + 2 * PAD_X) * scale
}

/// Box blur of `v` (`w` wide) with radius `r`, horizontally then vertically.
fn blur(v: &[f32], w: usize, r: usize) -> Vec<f32> {
    let h = v.len() / w;
    let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let mut out = vec![0.0; src.len()];
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0;
                let mut n = 0.0;
                for d in -(r as i32)..=r as i32 {
                    let (sx, sy) = if horizontal {
                        (x as i32 + d, y as i32)
                    } else {
                        (x as i32, y as i32 + d)
                    };
                    if sx >= 0 && sy >= 0 && (sx as usize) < w && (sy as usize) < h {
                        sum += src[sy as usize * w + sx as usize];
                        n += 1.0;
                    }
                }
                out[y * w + x] = sum / n;
            }
        }
        out
    };
    pass(&pass(v, true), false)
}

/// A strip of `tape` with `text` punched into it, letters `scale` screen pixels per font pixel.
/// The returned sprite is centred on the tape and carries its own shadow.
pub fn strip(text: &str, tape: [f32; 3], scale: usize, seed: u32) -> Rgba {
    let cols = columns(text);
    let cell = scale * SS;
    let (lw, lh) = ((cols.len() + 2 * PAD_X) * cell, (7 + 2 * PAD_Y) * cell);

    // Letter mask, then rounded into embossed domes.
    let mut mask = vec![0.0; lw * lh];
    for (cx, col) in cols.iter().enumerate() {
        for (cy, &lit) in col.iter().enumerate() {
            if lit {
                for y in 0..cell {
                    for x in 0..cell {
                        mask[((PAD_Y + cy) * cell + y) * lw + (PAD_X + cx) * cell + x] = 1.0;
                    }
                }
            }
        }
    }
    let letters = blur(&mask, lw, cell / 3);

    // Height: a bevelled tape with rounded corners, letters standing proud of it.
    let corner = cell as f32 * 0.9;
    let bevel = cell as f32 * 0.6;
    let edge = |x: f32, y: f32| -> f32 {
        let qx = (x - lw as f32 / 2.0).abs() - (lw as f32 / 2.0 - corner);
        let qy = (y - lh as f32 / 2.0).abs() - (lh as f32 / 2.0 - corner);
        let out = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
        corner - out - qx.max(qy).min(0.0)
    };
    let height = |x: usize, y: usize| -> f32 {
        let e = edge(x as f32 + 0.5, y as f32 + 0.5);
        0.6 * smooth(0.0, bevel, e) + 0.9 * letters[y * lw + x]
    };
    let mut local = vec![[0.0f32; 4]; lw * lh];
    for y in 0..lh {
        for x in 0..lw {
            let e = edge(x as f32 + 0.5, y as f32 + 0.5);
            if e <= 0.0 {
                continue;
            }
            let h = height(x, y);
            let dx = height((x + 1).min(lw - 1), y) - height(x.saturating_sub(1), y);
            let dy = height(x, (y + 1).min(lh - 1)) - height(x, y.saturating_sub(1));
            let k = 2.6 / cell as f32 * SS as f32;
            let n = [-dx * k, -dy * k, 1.0];
            let l = (n[0] * n[0] + n[1] * n[1] + 1.0).sqrt();
            let n = n.map(|v| v / l);
            let lift = letters[y * lw + x];
            let (fx, fy) = (x as f32 / SS as f32, y as f32 / SS as f32);
            let grime = fbm(fx / 9.0, fy / 9.0, seed + 3, 3);
            let mut col = mix(
                tape,
                STRETCHED,
                smooth(0.15, 0.6, lift) * (0.85 + 0.15 * grime),
            );
            col = dim(col, 0.7 + 0.45 * facing(n).max(0.0) - 0.12 * grime);
            // Gloss: the tape curls a little, so a soft streak runs along its upper half.
            let v = y as f32 / lh as f32;
            let streak =
                (-((v - 0.3) / 0.12).powi(2)).exp() * (0.12 + 0.1 * fbm(fx / 14.0, 0.0, seed, 2));
            let spec = glint(n).max(0.0).powi(30) * 0.5 * (1.0 - h * 0.3) + streak * (1.0 - lift);
            col = mix(col, [255.0, 255.0, 250.0], spec.min(1.0));
            local[y * lw + x] = [col[0], col[1], col[2], smooth(0.0, 1.5, e)];
        }
    }

    // Stick it on crooked: rotate while averaging the supersamples down to screen pixels.
    let tilt = (hash(seed as i32, 7, 91) - 0.5) * 2.0 * 2.5f32.to_radians();
    let (s, c) = tilt.sin_cos();
    let (tw, th) = (lw as f32 / SS as f32, lh as f32 / SS as f32);
    let w = (tw * c.abs() + th * s.abs()).ceil() as usize + 2 * MARGIN;
    let h = (tw * s.abs() + th * c.abs()).ceil() as usize + 2 * MARGIN;
    let sample = |px: f32, py: f32| -> [f32; 4] {
        let (rx, ry) = (px - w as f32 / 2.0, py - h as f32 / 2.0);
        let (ux, uy) = (rx * c + ry * s + tw / 2.0, -rx * s + ry * c + th / 2.0);
        let (lx, ly) = ((ux * SS as f32) as i32, (uy * SS as f32) as i32);
        if lx < 0 || ly < 0 || lx >= lw as i32 || ly >= lh as i32 {
            return [0.0; 4];
        }
        local[ly as usize * lw + lx as usize]
    };
    let mut px = vec![[0.0f32; 4]; w * h];
    let mut cover = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let p = sample(
                        x as f32 + (sx as f32 + 0.5) / SS as f32,
                        y as f32 + (sy as f32 + 0.5) / SS as f32,
                    );
                    for i in 0..3 {
                        acc[i] += p[i] * p[3];
                    }
                    acc[3] += p[3];
                }
            }
            let a = acc[3] / (SS * SS) as f32;
            cover[y * w + x] = a;
            if acc[3] > 0.0 {
                px[y * w + x] = [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], a];
            }
        }
    }

    // The shadow: the tape's outline, shifted away from the bulb and softened, under the tape.
    let shadow = blur(&cover, w, 2);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x as i32 - SHADOW.0 as i32, y as i32 - SHADOW.1 as i32);
            if sx < 0 || sy < 0 {
                continue;
            }
            let sa = shadow[sy as usize * w + sx as usize] * 0.45;
            let p = &mut px[y * w + x];
            let a = p[3] + sa * (1.0 - p[3]);
            if a > 0.0 {
                for i in 0..3 {
                    p[i] = p[i] * p[3] / a;
                }
            }
            p[3] = a;
        }
    }
    Rgba { w, px }
}

/// Composite a strip centred on `(cx, cy)` into a true-colour `W`-wide image.
pub fn bake(img: &mut [[f32; 3]], w: usize, strip: &Rgba, cx: i32, cy: i32) {
    let h = img.len() / w;
    let (x0, y0) = (
        cx - strip.w as i32 / 2,
        cy - (strip.px.len() / strip.w) as i32 / 2,
    );
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
