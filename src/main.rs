//! SPORAGON: Hexxagon in the back of a forgotten fridge. Your Penicillium against the
//! computer's Rhizopus, on a cracked egg tray, in VESA 640x480x256.
//!
//! Time is counted in frames (`Sporagon::t`, one per [`DosGame::step`], at the display's
//! refresh rate). Positions are screen pixels unless they are cell indices into
//! [`board::geometry`].

mod art;
mod board;

use art::{BRIDGE, Rgba, Rng, SPR, STAGES, Scene, slime};
use board::{Board, Cell, Move, Side, geometry};
use game_toolkit_dosvideo::prelude::*;
use std::collections::HashMap;

/// The side the human plays, unless in demo mode.
const PLAYER: Side = Side::Peni;

// What the black label strips say, one strip per line.

/// While the colonies of a move grow in.
const MSG_GROWING: &str = "SOMETHING\nGROWS...";
/// The human's turn.
const MSG_YOUR_MOVE: &str = "YOUR MOVE\nPICK MOULD\nTHEN A CUP";
/// The computer is about to move for Penicillium (demo mode).
const MSG_PENI_THINKS: &str = "PENICILLIUM\nSPREADS...";
/// The computer is about to move for Rhizopus.
const MSG_RHIZO_THINKS: &str = "RHIZOPUS\nSPREADS...";
/// Game over, the human has more colonies.
const MSG_WIN: &str = "YOU WIN!\nTHE FRIDGE\nIS YOURS";
/// Game over, the computer has more colonies.
const MSG_LOSE: &str = "YOU LOSE\nRHIZOPUS\nOWNS IT";
/// Demo game over, Penicillium has more colonies.
const MSG_PENI_WINS: &str = "PENICILLIUM\nWINS";
/// Demo game over, Rhizopus has more colonies.
const MSG_RHIZO_WINS: &str = "RHIZOPUS\nWINS";
/// Game over with equal counts.
const MSG_DRAW: &str = "A DRAW\nNOBODY\nCLEANS UP";
/// Every message, for the test that they fit the label column.
#[cfg(test)]
const MESSAGES: [&str; 9] = [
    MSG_GROWING,
    MSG_YOUR_MOVE,
    MSG_PENI_THINKS,
    MSG_RHIZO_THINKS,
    MSG_WIN,
    MSG_LOSE,
    MSG_PENI_WINS,
    MSG_RHIZO_WINS,
    MSG_DRAW,
];
/// Plies the computer looks ahead.
const AI_DEPTH: u32 = 3;
/// Frames a colony takes to grow in.
const GROW: u64 = 26;
/// Frames a pseudopod takes to reach the next cup.
const CREEP: u64 = 16;
/// Frames a glob takes to gather, snap off and land two cups away.
const LEAP: u64 = 28;
/// Frames a colony takes to shrink away after leaping.
const VANISH: u64 = 12;
/// Frames the computer waits before it moves, so its turn can be followed.
const THINK: u32 = 35;

/// Slime on its way somewhere: a pseudopod (`leap: false`) or a flying glob.
#[derive(Clone, Copy)]
struct Ooze {
    /// Centre of the cup it sets out from, in screen pixels.
    from: (f32, f32),
    /// Centre of the cup it heads for, in screen pixels.
    to: (f32, f32),
    side: Side,
    /// Frame it sets out.
    start: u64,
    /// Frames until it arrives.
    len: u64,
    /// Drawn until `start + life`, so the slime stays until the new colony covers it.
    life: u64,
    /// A glob jumping two cups, rather than a pseudopod to the next one.
    leap: bool,
    /// Varies the shape of the glob.
    seed: u32,
}

/// A colony shrinking out of the cup it leapt from.
#[derive(Clone, Copy)]
struct Vanish {
    cell: usize,
    side: Side,
    /// Which sprite variant the colony was drawn with.
    variant: usize,
    /// Frame it starts to shrink; until then it is drawn full size.
    start: u64,
}

/// A colony growing in, over `GROW` frames from `start`.
#[derive(Clone, Copy)]
struct Anim {
    cell: usize,
    /// Frame it starts to grow; until then the cup shows only `under`, if anything.
    start: u64,
    /// The colony being overgrown, drawn underneath.
    under: Option<Side>,
}

/// Whose turn it is and what the game waits for.
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Waiting for the human's clicks.
    Player,
    /// Wait for the animations, then it is `Side`'s turn.
    Growing(Side),
    /// `Side`'s computer player waits this many more frames, then moves.
    Thinking(Side, u32),
    /// The game has ended; only a new game continues.
    Over,
}

/// A PC speaker sweep from `from` Hz to `to` Hz over `len` frames, `t` frames in.
#[derive(Clone, Copy)]
struct Sfx {
    from: f64,
    to: f64,
    len: u32,
    t: u32,
    /// Shake the pitch randomly by -30 % to +30 % every frame, for a wet sound.
    wobble: bool,
}

/// The whole game state.
struct Sporagon {
    /// Everything painted up front: background, palette, sprites.
    scene: Scene,
    /// Label strips made so far, by text and letter size.
    labels: HashMap<(String, usize), Rgba>,
    /// The frame being drawn, copied to the screen every step.
    frame: Bitmap,
    /// The mouse pointer sprite.
    cursor: Bitmap,
    board: Board,
    /// Per cell, which sprite variant its colony is drawn with.
    variant: Vec<usize>,
    phase: Phase,
    /// The human's colony picked to move, if any.
    selected: Option<usize>,
    /// Colonies growing in.
    anims: Vec<Anim>,
    /// Slime underway.
    oozes: Vec<Ooze>,
    /// Colonies shrinking after a jump.
    vanishing: Vec<Vanish>,
    /// Drives variants, sounds, the tube flicker and the computer's tie-breaks; the order of
    /// its calls is part of the game's behaviour.
    rng: Rng,
    /// Frames since start.
    t: u64,
    sfx: Option<Sfx>,
    /// The failing tube: a blink pattern and how far into it we are.
    flicker: Option<(&'static [u8], usize)>,
    /// The computer plays both sides.
    demo: bool,
    /// What the message strips say, one strip per line.
    message: &'static str,
}

/// Screen-pixel centre of cell `i`.
fn center(i: usize) -> (f32, f32) {
    let (q, r) = geometry().coords[i];
    art::cell_center(q, r)
}

/// Screen-pixel centre of every cell, by index, and whether it is a hole in the opening.
fn centers() -> Vec<(f32, f32, bool)> {
    let opening = Board::new();
    geometry()
        .coords
        .iter()
        .zip(&opening.cells)
        .map(|(&(q, r), &c)| {
            let (x, y) = art::cell_center(q, r);
            (x, y, c == Cell::Hole)
        })
        .collect()
}

impl Sporagon {
    /// Reset to the opening, with fresh colony variants, the starting colonies growing in, and
    /// the human (or in demo mode the computer for `PLAYER`) to move once they have.
    fn new_game(&mut self) {
        self.board = Board::new();
        self.variant = (0..self.board.cells.len())
            .map(|_| self.rng.next() as usize % art::VARIANTS)
            .collect();
        self.selected = None;
        self.oozes.clear();
        self.vanishing.clear();

        self.anims = (0..self.board.cells.len())
            .filter(|&i| matches!(self.board.cells[i], Cell::Mold(_)))
            .map(|cell| Anim {
                cell,
                start: self.t,
                under: None,
            })
            .collect();

        self.phase = Phase::Growing(PLAYER);
        self.message = MSG_GROWING;
    }

    /// The cell under screen pixel `(x, y)`, if any.
    fn cell_at(&self, x: i32, y: i32) -> Option<usize> {
        let (q, r) = art::pixel_to_cell(x as f32, y as f32);
        board::index_of(q, r)
    }

    /// Play the legal move `mv` for `side` on the board and schedule how it looks and sounds,
    /// then wait for that to finish before `side.other()` moves.
    ///
    /// The schedule, in frames from now (`t`):
    /// - A jump: the glob flies for `LEAP` frames; the old colony starts shrinking 6 frames in,
    ///   and the new one starts growing at `t + LEAP - 3`, just
    ///   before landing, so it covers the glob (kept 2 frames past landing).
    /// - A clone: the pseudopod creeps for `CREEP` frames and the new colony starts at
    ///   `t + CREEP - 4`; the pseudopod stays for `GROW` more frames, under the colony.
    /// - Each takeover, the `k`-th from `0`: a 10-frame pseudopod from the new colony starts
    ///   at `arrive + 4 + 3 k`, and 7 frames in, the colony regrows over the old one.
    ///
    /// The board changes at once; only the drawing lags behind it.
    fn play(&mut self, side: Side, mv: Move) {
        let t = self.t;
        let old_variant = self.variant[mv.from];
        let converted = self.board.apply(side, mv);
        self.variant[mv.to] = self.rng.next() as usize % art::VARIANTS;
        let (from, to) = (center(mv.from), center(mv.to));
        let seed = self.rng.next();

        // The frame the new colony starts growing.
        let arrive = if mv.jump {
            self.vanishing.push(Vanish {
                cell: mv.from,
                side,
                variant: old_variant,
                start: t + 6,
            });
            self.oozes.push(Ooze {
                from,
                to,
                side,
                start: t,
                len: LEAP,
                life: LEAP + 2,
                leap: true,
                seed,
            });
            t + LEAP - 3
        } else {
            self.oozes.push(Ooze {
                from,
                to,
                side,
                start: t,
                len: CREEP,
                life: CREEP + GROW,
                leap: false,
                seed,
            });
            t + CREEP - 4
        };
        self.anims.push(Anim {
            cell: mv.to,
            start: arrive,
            under: None,
        });

        // From the new colony, a pseudopod into every neighbour it takes over.
        for (k, &cell) in converted.iter().enumerate() {
            let start = arrive + 4 + k as u64 * 3;
            self.oozes.push(Ooze {
                from: to,
                to: center(cell),
                side,
                start,
                len: 10,
                life: 10 + GROW,
                leap: false,
                seed,
            });
            self.anims.push(Anim {
                cell,
                start: start + 7,
                under: Some(side.other()),
            });
        }

        // A rising chirp for a jump, a low gurgle for a clone, and for takeovers a sweep from
        // 140 Hz that ends higher and lasts longer the more colonies are taken.
        self.sfx = Some(if converted.is_empty() {
            if mv.jump {
                Sfx {
                    from: 250.0,
                    to: 900.0,
                    len: 14,
                    t: 0,
                    wobble: false,
                }
            } else {
                Sfx {
                    from: 90.0,
                    to: 170.0,
                    len: CREEP as u32,
                    t: 0,
                    wobble: true,
                }
            }
        } else {
            Sfx {
                from: 140.0,
                to: 60.0 + 40.0 * converted.len() as f64,
                len: 10 + 4 * converted.len() as u32,
                t: 0,
                wobble: true,
            }
        });

        self.selected = None;
        self.phase = Phase::Growing(side.other());
    }

    /// Start `side`'s turn, or end the game if it cannot move.
    ///
    /// Without moves, the opponent overgrows every empty cup (two frames apart, in cell
    /// order), the result goes up on the label and the phase becomes `Over`. Otherwise the
    /// human gets the turn (`Player`) if it is theirs, and the computer `THINK` frames of
    /// pause (`Thinking`) if not.
    fn begin_turn(&mut self, side: Side) {
        if !self.board.has_moves(side) {
            for (k, cell) in self.board.fill_for(side.other()).into_iter().enumerate() {
                self.anims.push(Anim {
                    cell,
                    start: self.t + k as u64 * 2,
                    under: None,
                });
            }

            let player_count = self.board.count(PLAYER);
            let computer_count = self.board.count(PLAYER.other());
            self.message = match (player_count.cmp(&computer_count), self.demo) {
                (std::cmp::Ordering::Greater, false) => MSG_WIN,
                (std::cmp::Ordering::Less, false) => MSG_LOSE,
                (std::cmp::Ordering::Greater, true) => MSG_PENI_WINS,
                (std::cmp::Ordering::Less, true) => MSG_RHIZO_WINS,
                (std::cmp::Ordering::Equal, _) => MSG_DRAW,
            };
            self.sfx = Some(Sfx {
                from: 200.0,
                to: 50.0,
                len: 40,
                t: 0,
                wobble: true,
            });
            self.phase = Phase::Over;
        } else if side == PLAYER && !self.demo {
            self.message = MSG_YOUR_MOVE;
            self.phase = Phase::Player;
        } else {
            self.message = if side == PLAYER {
                MSG_PENI_THINKS
            } else {
                MSG_RHIZO_THINKS
            };
            self.phase = Phase::Thinking(side, THINK);
        }
    }

    /// The human clicked on `cell` (`None`: off the board). Clicking an own colony selects it,
    /// or deselects it if it was selected; then clicking an empty cup one or two away plays the
    /// move. Anything else clears the selection.
    fn handle_click(&mut self, cell: Option<usize>) {
        let Some(clicked) = cell else {
            self.selected = None;
            return;
        };

        match (self.selected, self.board.cells[clicked]) {
            (_, Cell::Mold(side)) if side == PLAYER => {
                self.selected = (self.selected != Some(clicked)).then_some(clicked);
                self.sfx = Some(Sfx {
                    from: 1200.0,
                    to: 1200.0,
                    len: 2,
                    t: 0,
                    wobble: false,
                });
            }
            (Some(from), Cell::Empty) => match self.board.legal(PLAYER, from, clicked) {
                Some(mv) => self.play(PLAYER, mv),
                None => self.selected = None,
            },
            _ => self.selected = None,
        }
    }

    /// Advance the current sweep by one frame, silencing the speaker when it ends.
    fn update_sound(&mut self, dos: &mut Dos) {
        let Some(sfx) = &mut self.sfx else { return };
        if sfx.t >= sfx.len {
            dos.nosound();
            self.sfx = None;
            return;
        }

        let progress = sfx.t as f64 / sfx.len as f64;
        let mut hz = sfx.from + (sfx.to - sfx.from) * progress;
        if sfx.wobble {
            hz *= 0.7 + 0.6 * (self.rng.f() as f64);
        }
        dos.sound(hz);
        sfx.t += 1;
    }

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
    fn draw(&mut self, mouse: (i32, i32), hover: Option<usize>) {
        let frame = &mut self.frame;
        frame.data.copy_from_slice(&self.scene.background.data);
        let geo = geometry();

        // --- 1. Move hints under the colonies: empty cups to clone and to jump to. ---
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

        // --- 2. Slime puddles under the colonies. ---
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

        // --- 3. Slime bridges, centred between the two cups; the pair picks the variant. ---
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

        // --- 4. Pseudopods creeping to the next cup, eased in and out. ---
        for ooze in self.oozes.iter().filter(|o| !o.leap && t >= o.start) {
            let progress = art::smooth(0.0, 1.0, (t - ooze.start) as f32 / ooze.len as f32);
            slime::draw(
                frame,
                quant,
                &slime::creep(ooze.from, ooze.to, progress, t as f32),
                ooze.side as usize,
            );
        }

        // --- 5. The colonies. ---
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

        // --- 6. Necks: where neighbours of one kind touch, the mould grows across: one mass,
        // not two. All rims first, so no rim covers another join's core. ---
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

        // --- 7. Globs in flight go over everything on the tray. ---
        for ooze in self.oozes.iter().filter(|o| o.leap && t >= o.start) {
            let progress = ((t - ooze.start) as f32 / ooze.len as f32).min(1.0);
            let (glob, shadow, trail) = slime::leap(ooze.from, ooze.to, progress, ooze.seed);
            if let Some(shadow) = shadow {
                slime::shadow(frame, quant, shadow, 0.45);
            }
            slime::draw(frame, quant, &trail, ooze.side as usize);
            slime::draw(frame, quant, &glob, ooze.side as usize);
        }

        // --- 8. The selected colony blinks; on the human's turn, an own colony under the
        // pointer gets a thin outline. ---
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

        // --- 9. The labels that change; the rest are part of the wall. ---
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

        // --- 10. The pointer, on top of everything. ---
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

impl DosGame for Sporagon {
    /// Switch to 640x480x256, paint the scene, load its palette and start the first game.
    fn init(dos: &mut Dos) -> anyhow::Result<Self> {
        dos.vbe_set_mode(0x101)
            .ok_or_else(|| anyhow::anyhow!("VBE mode 101h (640x480x256) not available"))?;
        // INT 33h fn 01h: we draw the pointer ourselves, so the host hides the OS one.
        dos.mouse_mut().show();
        dos.vbe_set_dac_width(8);

        let scene = art::paint(&centers());
        dos.set_palette_range(0, &scene.palette);
        let cursor = Bitmap::from_art(
            &[
                "X.........",
                "XX........",
                "XWX.......",
                "XWWX......",
                "XWWWX.....",
                "XWWWWX....",
                "XWWWWWX...",
                "XWWWWWWX..",
                "XWWWWWWWX.",
                "XWWWWXXXXX",
                "XWWXWX....",
                "XWX.XWX...",
                "XX..XWX...",
                ".....XWX..",
                ".....XX...",
            ],
            &[('.', art::TRANSPARENT), ('X', art::INK), ('W', art::WHITE)],
        );

        let mut game = Self {
            scene,
            labels: HashMap::new(),
            frame: Bitmap::new(art::W as u32, art::H as u32),
            cursor,
            board: Board::new(),
            variant: Vec::new(),
            phase: Phase::Player,
            selected: None,
            anims: Vec::new(),
            oozes: Vec::new(),
            vanishing: Vec::new(),
            rng: Rng(0x1234_5678_9ABC_DEF1),
            t: 0,
            sfx: None,
            flicker: None,
            demo: false,
            message: "",
        };
        game.new_game();
        Ok(game)
    }

    /// One frame:
    ///
    /// 1. keys: Esc quits, N starts over, D toggles demo mode (handing the turn to the
    ///    computer if the human was to move);
    /// 2. the clock ticks and finished animations, oozes and vanishings are dropped;
    /// 3. the phase advances: a click on the human's turn, the next turn once nothing is
    ///    moving on the tray any more, or the computer's move once its pause has run out;
    /// 4. sound, drawing and the copy to the screen on the vertical retrace;
    /// 5. the fluorescent tube, which now and then (1 in 500 per frame) plays a blink pattern
    ///    by dimming the palette.
    fn step(&mut self, dos: &mut Dos) {
        while let Some(key) = dos.getch() {
            match key.scan {
                sc::ESC => dos.exit(),
                sc::N => self.new_game(),
                sc::D => {
                    self.demo = !self.demo;
                    if self.phase == Phase::Player {
                        self.selected = None;
                        self.begin_turn(PLAYER);
                    }
                }
                _ => {}
            }
        }
        let mouse = dos.mouse_pixel();
        let hover = self.cell_at(mouse.0, mouse.1);
        let clicks = dos.mouse_mut().take_presses(0);
        self.t += 1;

        let t = self.t;
        self.anims.retain(|a| t < a.start + GROW);
        self.oozes.retain(|o| t < o.start + o.life);
        self.vanishing.retain(|v| t < v.start + VANISH);

        match self.phase {
            Phase::Player if clicks > 0 => self.handle_click(hover),
            Phase::Growing(side)
                if self.anims.is_empty() && self.oozes.is_empty() && self.vanishing.is_empty() =>
            {
                self.begin_turn(side)
            }
            Phase::Thinking(side, 0) => {
                // The search gets its own generator, seeded once from the game's, so its
                // number of draws does not shift the game's sequence.
                let mut rng = Rng(self.rng.next() as u64 | 1);
                match board::best_move(&self.board, side, AI_DEPTH, || rng.next()) {
                    Some(mv) => self.play(side, mv),
                    None => self.begin_turn(side),
                }
            }
            Phase::Thinking(side, wait) => self.phase = Phase::Thinking(side, wait - 1),
            _ => {}
        }
        self.update_sound(dos);

        self.draw(mouse, hover);
        dos.wait_vretrace();
        dos.vbe_write_lfb(0, &self.frame.data);

        // The fridge tube is on its way out: now and then it blinks off and strikes again.
        if self.flicker.is_none() && self.rng.next().is_multiple_of(500) {
            let k = self.rng.next() as usize % BLINKS.len();
            self.flicker = Some((BLINKS[k], 0));
        }
        if let Some((pattern, i)) = self.flicker {
            // Palette brightness, out of 64.
            let level = match pattern.get(i) {
                Some(b'#') => 22,
                Some(b'-') => 46,
                _ => 64,
            };
            dos.fade_palette(&self.scene.palette, level);

            // A tick as the tube strikes again, unless a move sound is playing.
            let strikes = i > 0 && pattern[i - 1] == b'#' && level > 22;
            if self.sfx.is_none() {
                if strikes {
                    dos.sound(90.0);
                } else {
                    dos.nosound();
                }
            }
            // One frame past the end at full brightness, then done.
            self.flicker = (i < pattern.len()).then_some((pattern, i + 1));
        }
    }
}

/// Frames of a failing fluorescent tube: `#` off, `-` struggling, `.` on.
const BLINKS: &[&[u8]] = &[
    b"###..",
    b"##.##-.",
    b"#.#.###--.",
    b"####-.....##-",
    b"#..#....###--",
];

/// The controls, as `(key, what it does)`, handed to the host to show.
const KEYS: &[(&str, &str)] = &[
    (
        "Mouse",
        "click your colony, then a cup: next to it clones, two away jumps",
    ),
    ("N", "new game"),
    ("D", "demo: the computer plays both sides"),
    ("Esc", "quit"),
];

/// Run the game on the toolkit's DOS-style SVGA host.
fn main() -> anyhow::Result<()> {
    run::<Sporagon>(
        DosConfig::new("SPORAGON", Adapter::Svga { memory_kib: 1024 })
            .with_bus(BusTiming::PCI)
            .with_keys(KEYS),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_fits_the_label_column() {
        for line in MESSAGES.iter().flat_map(|m| m.lines()) {
            assert!(
                art::dymo::width(line, 2) <= art::HUD_W,
                "{line:?} is too long"
            );
        }
        for name in art::SIDE_NAMES.iter().chain([&art::KEYS, &art::GAME_TITLE]) {
            assert!(
                art::dymo::width(name, 2) <= art::HUD_W,
                "{name:?} is too long"
            );
        }
    }
}
