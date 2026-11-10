//! Composing a frame: the wall, slime and moulds in their layers, the HUD labels that change,
//! and the pointer.
//!
//! Each layer is a free function that gets only the fields it reads plus the frame (and the
//! quantiser, for blended slime) it draws into. As `&self` methods they could not borrow the
//! frame mutably while reading the rest of the game.

use super::*;

/// A colony as it appears in a cup this frame, which can differ from the board while
/// animations play.
#[derive(Clone, Copy)]
struct Shown {
    cell: usize,
    side: Side,
    /// Which of the sprite variants this colony uses.
    variant: usize,
    /// Growth stage, `0..STAGES`; `STAGES - 1` is full size.
    stage: usize,
}

/// Two neighbouring cups showing the same kind of mould this frame, joined by slime under
/// the colonies and by a mould neck over them.
#[derive(Clone, Copy)]
struct Join {
    cell: usize,
    neighbour: usize,
    /// Index into [`art::BRIDGE_DIRS`]: the direction from `cell` to `neighbour`.
    direction: usize,
    side: Side,
}

impl Join {
    /// Sprite variant for this pair; fixed per pair, so a join does not flicker.
    fn variant(&self) -> usize {
        (self.cell * 7 + self.neighbour * 3) % art::BRIDGE_VARIANTS
    }

    /// Top-left screen pixel of a `BRIDGE`-sized sprite centred between the two cups.
    fn top_left(&self) -> (i32, i32) {
        let ((x0, y0), (x1, y1)) = (center(self.cell), center(self.neighbour));
        let half = BRIDGE as i32 / 2;
        (
            ((x0 + x1) / 2.0) as i32 - half,
            ((y0 + y1) / 2.0) as i32 - half,
        )
    }
}

impl Sporagon {
    /// Draw the frame over the wall and tray background, from the bottom layer up.
    ///
    /// `hover` is the cell under the pointer. Everything goes by what is on screen right now
    /// (animations included), not just the board, which runs ahead of it.
    pub(super) fn draw(&mut self, mouse: (i32, i32), hover: Option<usize>) {
        let shown = self.shown_colonies();
        let joins = self.joins();
        let t = self.t;

        let frame = &mut self.frame;
        let quant = &mut self.scene.quant;
        frame.data.copy_from_slice(&self.scene.background.data);

        draw_move_hints(frame, &self.board, self.selected, hover, t);
        draw_puddles(frame, quant, &self.scene.puddles, &shown);
        draw_bridges(frame, quant, &self.scene.bridges, &joins);
        draw_pseudopods(frame, quant, &self.oozes, t);
        draw_colonies(frame, &self.scene.colonies, &shown);
        draw_necks(frame, quant, &self.scene.necks, &joins);
        draw_globs(frame, quant, &self.oozes, t);

        let pickable_hover = hover.filter(|_| self.phase == Phase::Player);
        draw_selection(frame, &self.board, self.selected, pickable_hover, t);
        draw_hud(
            frame,
            quant,
            &mut self.labels,
            &self.scene.colonies,
            &self.board,
            self.demo,
            self.message,
        );

        // The pointer, on top of everything.
        frame.blit(&self.cursor, mouse.0, mouse.1, Some(art::TRANSPARENT));
    }

    /// What each cup shows, bottom layer first.
    ///
    /// - A colony being overgrown shows the old side at full size under the new one.
    /// - A growing colony is at the stage its age gives it, and not drawn before it starts.
    /// - A colony that leapt away shrinks back through the stages over `VANISH` frames.
    fn shown_colonies(&self) -> Vec<Shown> {
        let t = self.t;
        let mut shown = Vec::new();

        for cell in 0..self.board.cells.len() {
            let Cell::Mold(side) = self.board.cells[cell] else {
                continue;
            };
            let variant = self.variant[cell];

            let anim = self.anims.iter().rev().find(|a| a.cell == cell);
            if let Some(under) = anim.and_then(|a| a.under) {
                shown.push(Shown {
                    cell,
                    side: under,
                    variant,
                    stage: STAGES - 1,
                });
            }
            if anim.is_none_or(|a| t >= a.start) {
                let stage = anim.map_or(STAGES - 1, |a| {
                    ((t - a.start) as usize * STAGES / GROW as usize).min(STAGES - 1)
                });
                shown.push(Shown {
                    cell,
                    side,
                    variant,
                    stage,
                });
            }
        }

        for vanish in &self.vanishing {
            let gone = t.saturating_sub(vanish.start) as usize * STAGES / VANISH as usize;
            if gone < STAGES {
                shown.push(Shown {
                    cell: vanish.cell,
                    side: vanish.side,
                    variant: vanish.variant,
                    stage: STAGES - 1 - gone,
                });
            }
        }
        shown
    }

    /// The mould kind cup `cell` shows for joining purposes this frame, if any.
    ///
    /// A colony that leapt away still counts until it starts to shrink. A colony being taken
    /// over keeps its old side until the new one has half overgrown it, so joins last until
    /// the takeover actually reaches them.
    fn joinable_side(&self, cell: usize) -> Option<Side> {
        let t = self.t;
        if let Some(vanish) = self.vanishing.iter().find(|v| v.cell == cell) {
            return (t < vanish.start).then_some(vanish.side);
        }
        let Cell::Mold(side) = self.board.cells[cell] else {
            return None;
        };
        match self.anims.iter().rev().find(|a| a.cell == cell) {
            None => Some(side),
            Some(a) => a.under.filter(|_| t < a.start + GROW / 2),
        }
    }

    /// Every pair of neighbouring cups showing the same kind, each pair once (only the three
    /// [`art::BRIDGE_DIRS`] directions are looked at from each cell).
    fn joins(&self) -> Vec<Join> {
        let geo = geometry();
        let mut joins = Vec::new();

        for cell in 0..self.board.cells.len() {
            let Some(side) = self.joinable_side(cell) else {
                continue;
            };
            let (q, r) = geo.coords[cell];
            for (direction, (dq, dr)) in art::BRIDGE_DIRS.iter().enumerate() {
                if let Some(neighbour) = board::index_of(q + dq, r + dr)
                    && self.joinable_side(neighbour) == Some(side)
                {
                    joins.push(Join {
                        cell,
                        neighbour,
                        direction,
                        side,
                    });
                }
            }
        }
        joins
    }
}

/// Under the colonies: around the selected colony, outline the empty cups it can clone to
/// (near) and jump to (far). They pulse every 8 frames; the hovered one stays thick.
fn draw_move_hints(
    frame: &mut Bitmap,
    board: &Board,
    selected: Option<usize>,
    hover: Option<usize>,
    t: u64,
) {
    let Some(selected) = selected else {
        return;
    };
    let geo = geometry();
    let pulse = (t / 8).is_multiple_of(2);
    let rings = [
        (&geo.near[selected], art::GLOW_NEAR),
        (&geo.far[selected], art::GLOW_FAR),
    ];

    for &(cells, colour) in &rings {
        for &cell in cells {
            if board.cells[cell] == Cell::Empty {
                let (x, y) = center(cell);
                let thick = pulse || Some(cell) == hover;
                hex_outline(frame, x, y, art::R - 5.0, colour, thick);
            }
        }
    }
}

/// The translucent mucus puddle under every shown colony, sized to its stage.
fn draw_puddles(
    frame: &mut Bitmap,
    quant: &mut art::Quantizer,
    puddles: &[Vec<Vec<Rgba>>; 2],
    shown: &[Shown],
) {
    for colony in shown {
        let (x, y) = center(colony.cell);
        let puddle = &puddles[colony.side as usize][colony.variant][colony.stage];
        slime::blit(
            frame,
            quant,
            puddle,
            x as i32 - SPR as i32 / 2,
            y as i32 - SPR as i32 / 2,
        );
    }
}

/// Slime bridges across the ridge between joined cups, under the colonies.
fn draw_bridges(
    frame: &mut Bitmap,
    quant: &mut art::Quantizer,
    bridges: &[[Vec<Rgba>; 3]; 2],
    joins: &[Join],
) {
    for join in joins {
        let (x, y) = join.top_left();
        let bridge = &bridges[join.side as usize][join.direction][join.variant()];
        slime::blit(frame, quant, bridge, x, y);
    }
}

/// Pseudopods creeping to the next cup, eased in and out over their `len` frames.
fn draw_pseudopods(frame: &mut Bitmap, quant: &mut art::Quantizer, oozes: &[Ooze], t: u64) {
    for ooze in oozes.iter().filter(|o| !o.leap && t >= o.start) {
        let progress = art::smooth(0.0, 1.0, (t - ooze.start) as f32 / ooze.len as f32);
        let balls = slime::creep(ooze.from, ooze.to, progress, t as f32);
        slime::draw(frame, quant, &balls, ooze.side as usize);
    }
}

/// The colony sprites, centred on their cups, in `shown` order (old under new).
fn draw_colonies(frame: &mut Bitmap, colonies: &[Vec<Vec<Bitmap>>; 2], shown: &[Shown]) {
    for colony in shown {
        let (x, y) = center(colony.cell);
        let (sx, sy) = (x as i32 - SPR as i32 / 2, y as i32 - SPR as i32 / 2);
        let sprite = &colonies[colony.side as usize][colony.variant][colony.stage];
        frame.blit(sprite, sx, sy, Some(art::TRANSPARENT));
    }
}

/// Mould grown across the ridge between joined colonies, so neighbours read as one mass.
/// Every rim goes down before any core, so no rim covers another join's core where joins
/// cross inside a mass.
fn draw_necks(
    frame: &mut Bitmap,
    quant: &mut art::Quantizer,
    necks: &[[Vec<(Rgba, Rgba)>; 3]; 2],
    joins: &[Join],
) {
    for core in [false, true] {
        for join in joins {
            let (x, y) = join.top_left();
            let (rim, inner) = &necks[join.side as usize][join.direction][join.variant()];
            let layer = if core { inner } else { rim };
            slime::blit(frame, quant, layer, x, y);
        }
    }
}

/// Globs in flight, over everything on the tray: the shadow on the tray, then the trail of
/// drops, then the glob itself.
fn draw_globs(frame: &mut Bitmap, quant: &mut art::Quantizer, oozes: &[Ooze], t: u64) {
    for ooze in oozes.iter().filter(|o| o.leap && t >= o.start) {
        let progress = ((t - ooze.start) as f32 / ooze.len as f32).min(1.0);
        let (glob, shadow, trail) = slime::leap(ooze.from, ooze.to, progress, ooze.seed);
        if let Some(shadow) = shadow {
            slime::shadow(frame, quant, shadow, 0.45);
        }
        slime::draw(frame, quant, &trail, ooze.side as usize);
        slime::draw(frame, quant, &glob, ooze.side as usize);
    }
}

/// The selected colony blinks (every 6 frames). Without a selection, an own colony under the
/// pointer gets a thin outline; `pickable_hover` is `None` when the human cannot pick now.
fn draw_selection(
    frame: &mut Bitmap,
    board: &Board,
    selected: Option<usize>,
    pickable_hover: Option<usize>,
    t: u64,
) {
    if let Some(selected) = selected {
        let (x, y) = center(selected);
        let colour = if (t / 6).is_multiple_of(2) {
            art::WHITE
        } else {
            art::GLOW_FAR
        };
        hex_outline(frame, x, y, art::R - 3.0, colour, true);
    } else if let Some(hovered) = pickable_hover
        && board.cells[hovered] == Cell::Mold(PLAYER)
    {
        let (x, y) = center(hovered);
        hex_outline(frame, x, y, art::R - 3.0, art::WHITE, false);
    }
}

/// The HUD labels that change; the title, the mould names and the keys strip are part of the
/// wall already. For each side, a colony icon with its count and who plays it; below, one
/// strip per message line, every other one stuck a little further right.
fn draw_hud(
    frame: &mut Bitmap,
    quant: &mut art::Quantizer,
    labels: &mut HashMap<(String, usize), Rgba>,
    colonies: &[Vec<Vec<Bitmap>>; 2],
    board: &Board,
    demo: bool,
    message: &str,
) {
    for side in [Side::Peni, Side::Rhizo] {
        let y = art::SIDE_Y[side as usize];
        frame.blit(
            &colonies[side as usize][0][STAGES - 1],
            art::HUD_X + 2,
            y + 12,
            Some(art::TRANSPARENT),
        );

        let score = board.count(side).to_string();
        label(labels, frame, quant, &score, 3, (art::HUD_X + 102, y + 36));

        let who = if side == PLAYER && !demo {
            "YOU"
        } else {
            "CPU"
        };
        label(labels, frame, quant, who, 2, (art::HUD_X + 102, y + 70));
    }

    for (k, line) in message.lines().enumerate() {
        let x = art::HUD_CX + if k % 2 == 0 { -2 } else { 3 };
        let y = art::MESSAGE_Y + k as i32 * art::MESSAGE_STEP;
        label(labels, frame, quant, line, 2, (x, y));
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
