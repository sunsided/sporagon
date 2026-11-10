//! Composing a frame: the wall, slime and moulds in their layers, the HUD labels that change,
//! and the pointer.

use super::*;

impl Sporagon {
    /// Draw the frame: the wall and tray background, then from the bottom up:
    ///
    /// 1. move hints around the selected colony;
    /// 2. slime puddles under the colonies;
    /// 3. slime bridges between neighbouring colonies of one kind;
    /// 4. creeping pseudopods;
    /// 5. the colonies;
    /// 6. mould necks joining neighbouring colonies, rim first, then core;
    /// 7. leaping globs with their shadows and trails;
    /// 8. the selection or hover outline;
    /// 9. the HUD labels that change: colony counts, who plays, the message;
    /// 10. the mouse pointer at `mouse`.
    ///
    /// `hover` is the cell under the pointer. Everything goes by what is on screen right now
    /// (animations included), not just the board, which runs ahead of it.
    pub(super) fn draw(&mut self, mouse: (i32, i32), hover: Option<usize>) {
        let frame = &mut self.frame;
        frame.data.copy_from_slice(&self.scene.background.data);
        let geo = geometry();

        // 1. Move hints under the colonies: empty cups to clone and to jump to.
        if let Some(selected) = self.selected {
            let pulse = (self.t / 8).is_multiple_of(2);
            let rings = [
                (&geo.near[selected], art::GLOW_NEAR),
                (&geo.far[selected], art::GLOW_FAR),
            ];
            for &(list, colour) in &rings {
                for &j in list {
                    if self.board.cells[j] == Cell::Empty {
                        let (x, y) = center(j);
                        let thick = pulse || Some(j) == hover;
                        hex_outline(frame, x, y, art::R - 5.0, colour, thick);
                    }
                }
            }
        }

        // What each cup shows: `(cell, side, variant, stage)`, bottom layer first. A colony
        // being overgrown shows the old side at full size under the new one; a growing colony
        // is at the stage its age gives it; a vanishing one shrinks back through the stages.
        let t = self.t;
        let mut shown = Vec::new();
        for i in 0..self.board.cells.len() {
            let Cell::Mold(side) = self.board.cells[i] else {
                continue;
            };

            let anim = self.anims.iter().rev().find(|a| a.cell == i);
            if let Some(under) = anim.and_then(|a| a.under) {
                shown.push((i, under, self.variant[i], STAGES - 1));
            }
            if anim.is_none_or(|a| t >= a.start) {
                let stage = anim.map_or(STAGES - 1, |a| {
                    ((t - a.start) as usize * STAGES / GROW as usize).min(STAGES - 1)
                });
                shown.push((i, side, self.variant[i], stage));
            }
        }
        for vanish in &self.vanishing {
            let gone = t.saturating_sub(vanish.start) as usize * STAGES / VANISH as usize;
            if gone < STAGES {
                shown.push((vanish.cell, vanish.side, vanish.variant, STAGES - 1 - gone));
            }
        }

        // 2. Slime puddles under the colonies.
        let quant = &mut self.scene.quant;
        for &(i, side, variant, stage) in &shown {
            let (x, y) = center(i);
            let puddle = &self.scene.puddles[side as usize][variant][stage];
            slime::blit(
                frame,
                quant,
                puddle,
                x as i32 - SPR as i32 / 2,
                y as i32 - SPR as i32 / 2,
            );
        }

        // Neighbours showing the same kind right now, `(cell, neighbour, direction, side)`:
        // joined by slime under the colonies and by mould over them. Goes by what is on
        // screen, not the board, so joins last until a takeover actually reaches them.
        let shown_side = |i: usize| -> Option<Side> {
            if let Some(vanish) = self.vanishing.iter().find(|v| v.cell == i) {
                return (t < vanish.start).then_some(vanish.side);
            }
            let Cell::Mold(side) = self.board.cells[i] else {
                return None;
            };
            match self.anims.iter().rev().find(|a| a.cell == i) {
                None => Some(side),
                // The old colony holds until the new one has half overgrown it.
                Some(a) => a.under.filter(|_| t < a.start + GROW / 2),
            }
        };
        let mut joins = Vec::new();
        for i in 0..self.board.cells.len() {
            let Some(side) = shown_side(i) else {
                continue;
            };
            let (q0, r0) = geo.coords[i];
            for (d, (dq, dr)) in art::BRIDGE_DIRS.iter().enumerate() {
                if let Some(j) = board::index_of(q0 + dq, r0 + dr)
                    && shown_side(j) == Some(side)
                {
                    joins.push((i, j, d, side));
                }
            }
        }

        // 3. Slime bridges, centred between the two cups; the pair picks the variant.
        let half = BRIDGE as i32 / 2;
        for &(i, j, d, side) in &joins {
            let ((x0, y0), (x1, y1)) = (center(i), center(j));
            let variant = (i * 7 + j * 3) % art::BRIDGE_VARIANTS;
            let img = &self.scene.bridges[side as usize][d][variant];
            slime::blit(
                frame,
                quant,
                img,
                ((x0 + x1) / 2.0) as i32 - half,
                ((y0 + y1) / 2.0) as i32 - half,
            );
        }

        // 4. Pseudopods creeping to the next cup, eased in and out.
        for ooze in self.oozes.iter().filter(|o| !o.leap && t >= o.start) {
            let progress = art::smooth(0.0, 1.0, (t - ooze.start) as f32 / ooze.len as f32);
            slime::draw(
                frame,
                quant,
                &slime::creep(ooze.from, ooze.to, progress, t as f32),
                ooze.side as usize,
            );
        }

        // 5. The colonies.
        for &(i, side, variant, stage) in &shown {
            let (x, y) = center(i);
            let (sx, sy) = (x as i32 - SPR as i32 / 2, y as i32 - SPR as i32 / 2);
            frame.blit(
                &self.scene.colonies[side as usize][variant][stage],
                sx,
                sy,
                Some(art::TRANSPARENT),
            );
        }

        // 6. Necks: where neighbours of one kind touch, the mould grows across: one mass,
        // not two. All rims first, so no rim covers another join's core.
        for core in [false, true] {
            for &(i, j, d, side) in &joins {
                let ((x0, y0), (x1, y1)) = (center(i), center(j));
                let variant = (i * 7 + j * 3) % art::BRIDGE_VARIANTS;
                let (rim, inner) = &self.scene.necks[side as usize][d][variant];
                let img = if core { inner } else { rim };
                slime::blit(
                    frame,
                    quant,
                    img,
                    ((x0 + x1) / 2.0) as i32 - half,
                    ((y0 + y1) / 2.0) as i32 - half,
                );
            }
        }

        // 7. Globs in flight go over everything on the tray.
        for ooze in self.oozes.iter().filter(|o| o.leap && t >= o.start) {
            let progress = ((t - ooze.start) as f32 / ooze.len as f32).min(1.0);
            let (glob, shadow, trail) = slime::leap(ooze.from, ooze.to, progress, ooze.seed);
            if let Some(shadow) = shadow {
                slime::shadow(frame, quant, shadow, 0.45);
            }
            slime::draw(frame, quant, &trail, ooze.side as usize);
            slime::draw(frame, quant, &glob, ooze.side as usize);
        }

        // 8. The selected colony blinks; on the human's turn, an own colony under the
        // pointer gets a thin outline.
        if let Some(selected) = self.selected {
            let (x, y) = center(selected);
            let colour = if (self.t / 6).is_multiple_of(2) {
                art::WHITE
            } else {
                art::GLOW_FAR
            };
            hex_outline(frame, x, y, art::R - 3.0, colour, true);
        } else if let Some(hovered) = hover.filter(|_| self.phase == Phase::Player)
            && self.board.cells[hovered] == Cell::Mold(PLAYER)
        {
            let (x, y) = center(hovered);
            hex_outline(frame, x, y, art::R - 3.0, art::WHITE, false);
        }

        // 9. The labels that change; the rest are part of the wall.
        let (labels, quant) = (&mut self.labels, &mut self.scene.quant);
        for side in [Side::Peni, Side::Rhizo] {
            let y = art::SIDE_Y[side as usize];
            frame.blit(
                &self.scene.colonies[side as usize][0][STAGES - 1],
                art::HUD_X + 2,
                y + 12,
                Some(art::TRANSPARENT),
            );

            let score = self.board.count(side).to_string();
            label(labels, frame, quant, &score, 3, (art::HUD_X + 102, y + 36));

            let who = if side == PLAYER && !self.demo {
                "YOU"
            } else {
                "CPU"
            };
            label(labels, frame, quant, who, 2, (art::HUD_X + 102, y + 70));
        }
        // The message strips, every other one stuck a little further right.
        for (k, line) in self.message.lines().enumerate() {
            let x = art::HUD_CX + if k % 2 == 0 { -2 } else { 3 };
            let y = art::MESSAGE_Y + k as i32 * art::MESSAGE_STEP;
            label(labels, frame, quant, line, 2, (x, y));
        }

        // 10. The pointer, on top of everything.
        frame.blit(&self.cursor, mouse.0, mouse.1, Some(art::TRANSPARENT));
    }
}

/// Draw the black label strip for `text`, centred on screen pixel `at`, with letters `scale`
/// screen pixels per font pixel. Strips are made the first time and cached in `labels`; the
/// seed comes from the text, so a text always gets the same tilt.
fn label(
    labels: &mut HashMap<(String, usize), Rgba>,
    frame: &mut Bitmap,
    quant: &mut art::Quantizer,
    text: &str,
    scale: usize,
    at: (i32, i32),
) {
    let strip = labels.entry((text.to_string(), scale)).or_insert_with(|| {
        let seed = text
            .bytes()
            .fold(17u32, |hash, byte| hash.wrapping_mul(31) ^ byte as u32);
        art::dymo::strip(text, art::dymo::BLACK, scale, seed)
    });
    let strip_h = (strip.px.len() / strip.w) as i32;
    slime::blit(
        frame,
        quant,
        strip,
        at.0 - strip.w as i32 / 2,
        at.1 - strip_h / 2,
    );
}

/// Outline a pointy-topped hexagon of corner radius `radius` around `(x, y)` (screen pixels)
/// in palette colour `colour`; `thick` doubles each edge one pixel to the right.
fn hex_outline(frame: &mut Bitmap, x: f32, y: f32, radius: f32, colour: u8, thick: bool) {
    let corners: Vec<(i32, i32)> = (0..=6)
        .map(|i| {
            let angle = std::f32::consts::FRAC_PI_3 * i as f32 + std::f32::consts::FRAC_PI_6;
            (
                (x + radius * angle.cos()) as i32,
                (y + radius * angle.sin()) as i32,
            )
        })
        .collect();

    for edge in corners.windows(2) {
        let ((x0, y0), (x1, y1)) = (edge[0], edge[1]);
        frame.line(x0, y0, x1, y1, colour);
        if thick {
            frame.line(x0 + 1, y0, x1 + 1, y1, colour);
        }
    }
}
