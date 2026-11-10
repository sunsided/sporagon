//! SPORAGON: Hexxagon in the back of a forgotten fridge. Your Penicillium against the
//! computer's Rhizopus, on a cracked egg tray, in VESA 640x480x256.

mod art;
mod board;

use art::{BRIDGE, Rgba, Rng, SPR, STAGES, Scene, slime};
use board::{Board, Cell, Move, Side, geometry};
use game_toolkit_dosvideo::prelude::*;
use std::collections::HashMap;

const PLAYER: Side = Side::Peni;

// What the black label strips say, one strip per line.
const MSG_GROWING: &str = "SOMETHING\nGROWS...";
const MSG_YOUR_MOVE: &str = "YOUR MOVE\nPICK MOULD\nTHEN A CUP";
const MSG_PENI_THINKS: &str = "PENICILLIUM\nSPREADS...";
const MSG_RHIZO_THINKS: &str = "RHIZOPUS\nSPREADS...";
const MSG_WIN: &str = "YOU WIN!\nTHE FRIDGE\nIS YOURS";
const MSG_LOSE: &str = "YOU LOSE\nRHIZOPUS\nOWNS IT";
const MSG_PENI_WINS: &str = "PENICILLIUM\nWINS";
const MSG_RHIZO_WINS: &str = "RHIZOPUS\nWINS";
const MSG_DRAW: &str = "A DRAW\nNOBODY\nCLEANS UP";
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
const AI_DEPTH: u32 = 3;
/// Frames a colony takes to grow in.
const GROW: u64 = 26;
/// Frames a pseudopod takes to reach the next cup.
const CREEP: u64 = 16;
/// Frames a glob takes to gather, snap off and land two cups away.
const LEAP: u64 = 28;
/// Frames a colony takes to shrink away after leaping.
const VANISH: u64 = 12;

/// Slime on its way somewhere: a pseudopod (`leap: false`) or a flying glob.
#[derive(Clone, Copy)]
struct Ooze {
    from: (f32, f32),
    to: (f32, f32),
    side: Side,
    start: u64,
    len: u64,
    /// Drawn until `start + life`, so the slime stays until the new colony covers it.
    life: u64,
    leap: bool,
    seed: u32,
}

/// A colony shrinking out of the cup it leapt from.
#[derive(Clone, Copy)]
struct Vanish {
    cell: usize,
    side: Side,
    variant: usize,
    start: u64,
}

#[derive(Clone, Copy)]
struct Anim {
    cell: usize,
    start: u64,
    /// The colony being overgrown, drawn underneath.
    under: Option<Side>,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Player,
    /// Wait for the animations, then it is `Side`'s turn.
    Growing(Side),
    /// `Side`'s computer player waits a moment, then moves.
    Thinking(Side, u32),
    Over,
}

/// A PC speaker sweep: `(from Hz, to Hz, frames, wobble)`.
#[derive(Clone, Copy)]
struct Sfx {
    from: f64,
    to: f64,
    len: u32,
    t: u32,
    wobble: bool,
}

struct Sporagon {
    scene: Scene,
    /// Label strips made so far, by text and letter size.
    labels: HashMap<(String, usize), Rgba>,
    frame: Bitmap,
    cursor: Bitmap,
    board: Board,
    variant: Vec<usize>,
    phase: Phase,
    selected: Option<usize>,
    anims: Vec<Anim>,
    oozes: Vec<Ooze>,
    vanishing: Vec<Vanish>,
    rng: Rng,
    t: u64,
    sfx: Option<Sfx>,
    /// The failing tube: a blink pattern and how far into it we are.
    flicker: Option<(&'static [u8], usize)>,
    /// The computer plays both sides.
    demo: bool,
    message: &'static str,
}

fn center(i: usize) -> (f32, f32) {
    let (q, r) = geometry().coords[i];
    art::cell_center(q, r)
}

fn centers() -> Vec<(f32, f32, bool)> {
    let b = Board::new();
    geometry()
        .coords
        .iter()
        .zip(&b.cells)
        .map(|(&(q, r), &c)| {
            let (x, y) = art::cell_center(q, r);
            (x, y, c == Cell::Hole)
        })
        .collect()
}

impl Sporagon {
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

    fn cell_at(&self, x: i32, y: i32) -> Option<usize> {
        let (q, r) = art::pixel_to_cell(x as f32, y as f32);
        board::index_of(q, r)
    }

    fn play(&mut self, side: Side, mv: Move) {
        let t = self.t;
        let old_variant = self.variant[mv.from];
        let converted = self.board.apply(side, mv);
        self.variant[mv.to] = self.rng.next() as usize % art::VARIANTS;
        let (from, to) = (center(mv.from), center(mv.to));
        let seed = self.rng.next();
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
        for (k, &c) in converted.iter().enumerate() {
            let start = arrive + 4 + k as u64 * 3;
            self.oozes.push(Ooze {
                from: to,
                to: center(c),
                side,
                start,
                len: 10,
                life: 10 + GROW,
                leap: false,
                seed,
            });
            self.anims.push(Anim {
                cell: c,
                start: start + 7,
                under: Some(side.other()),
            });
        }
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
    fn begin_turn(&mut self, side: Side) {
        if !self.board.has_moves(side) {
            for (k, c) in self.board.fill_for(side.other()).into_iter().enumerate() {
                self.anims.push(Anim {
                    cell: c,
                    start: self.t + k as u64 * 2,
                    under: None,
                });
            }
            let (p, a) = (self.board.count(PLAYER), self.board.count(PLAYER.other()));
            self.message = match (p.cmp(&a), self.demo) {
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
            self.phase = Phase::Thinking(side, 35);
        }
    }

    fn handle_click(&mut self, cell: Option<usize>) {
        let Some(c) = cell else {
            self.selected = None;
            return;
        };
        match (self.selected, self.board.cells[c]) {
            (_, Cell::Mold(s)) if s == PLAYER => {
                self.selected = (self.selected != Some(c)).then_some(c);
                self.sfx = Some(Sfx {
                    from: 1200.0,
                    to: 1200.0,
                    len: 2,
                    t: 0,
                    wobble: false,
                });
            }
            (Some(from), Cell::Empty) => match self.board.legal(PLAYER, from, c) {
                Some(mv) => self.play(PLAYER, mv),
                None => self.selected = None,
            },
            _ => self.selected = None,
        }
    }

    fn update_sound(&mut self, dos: &mut Dos) {
        let Some(s) = &mut self.sfx else { return };
        if s.t >= s.len {
            dos.nosound();
            self.sfx = None;
            return;
        }
        let f = s.t as f64 / s.len as f64;
        let mut hz = s.from + (s.to - s.from) * f;
        if s.wobble {
            hz *= 0.7 + 0.6 * (self.rng.f() as f64);
        }
        dos.sound(hz);
        s.t += 1;
    }

    fn draw(&mut self, mouse: (i32, i32), hover: Option<usize>) {
        let f = &mut self.frame;
        f.data.copy_from_slice(&self.scene.background.data);
        let g = geometry();

        // Move hints under the colonies.
        if let Some(sel) = self.selected {
            let pulse = (self.t / 8).is_multiple_of(2);
            for &(list, col) in &[(&g.near[sel], art::GLOW_NEAR), (&g.far[sel], art::GLOW_FAR)] {
                for &j in list {
                    if self.board.cells[j] == Cell::Empty {
                        let (x, y) = center(j);
                        hex_outline(f, x, y, art::R - 5.0, col, pulse || Some(j) == hover);
                    }
                }
            }
        }

        // What each cup shows: `(cell, side, variant, stage)`, bottom layer first.
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
        for v in &self.vanishing {
            let gone = t.saturating_sub(v.start) as usize * STAGES / VANISH as usize;
            if gone < STAGES {
                shown.push((v.cell, v.side, v.variant, STAGES - 1 - gone));
            }
        }

        let q = &mut self.scene.quant;
        for &(i, side, variant, stage) in &shown {
            let (x, y) = center(i);
            let p = &self.scene.puddles[side as usize][variant][stage];
            slime::blit(
                f,
                q,
                p,
                x as i32 - SPR as i32 / 2,
                y as i32 - SPR as i32 / 2,
            );
        }
        // Neighbours showing the same kind right now, `(cell, neighbour, direction, side)`:
        // joined by slime under the colonies and by mould over them. Goes by what is on
        // screen, not the board, so joins last until a takeover actually reaches them.
        let shows = |i: usize| -> Option<Side> {
            if let Some(v) = self.vanishing.iter().find(|v| v.cell == i) {
                return (t < v.start).then_some(v.side);
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
            let Some(side) = shows(i) else {
                continue;
            };
            let (q0, r0) = g.coords[i];
            for (d, (dq, dr)) in art::BRIDGE_DIRS.iter().enumerate() {
                if let Some(j) = board::index_of(q0 + dq, r0 + dr)
                    && shows(j) == Some(side)
                {
                    joins.push((i, j, d, side));
                }
            }
        }
        for &(i, j, d, side) in &joins {
            let ((x0, y0), (x1, y1)) = (center(i), center(j));
            let img = &self.scene.bridges[side as usize][d][(i * 7 + j * 3) % art::BRIDGE_VARIANTS];
            let h = BRIDGE as i32 / 2;
            slime::blit(
                f,
                q,
                img,
                ((x0 + x1) / 2.0) as i32 - h,
                ((y0 + y1) / 2.0) as i32 - h,
            );
        }
        for o in self.oozes.iter().filter(|o| !o.leap && t >= o.start) {
            let p = art::smooth(0.0, 1.0, (t - o.start) as f32 / o.len as f32);
            slime::draw(
                f,
                q,
                &slime::creep(o.from, o.to, p, t as f32),
                o.side as usize,
            );
        }
        for &(i, side, variant, stage) in &shown {
            let (x, y) = center(i);
            let (sx, sy) = (x as i32 - SPR as i32 / 2, y as i32 - SPR as i32 / 2);
            f.blit(
                &self.scene.colonies[side as usize][variant][stage],
                sx,
                sy,
                Some(art::TRANSPARENT),
            );
        }
        // Where neighbours of one kind touch, the mould grows across: one mass, not two.
        for core in [false, true] {
            for &(i, j, d, side) in &joins {
                let ((x0, y0), (x1, y1)) = (center(i), center(j));
                let (rim, inner) =
                    &self.scene.necks[side as usize][d][(i * 7 + j * 3) % art::BRIDGE_VARIANTS];
                let h = BRIDGE as i32 / 2;
                let img = if core { inner } else { rim };
                slime::blit(
                    f,
                    q,
                    img,
                    ((x0 + x1) / 2.0) as i32 - h,
                    ((y0 + y1) / 2.0) as i32 - h,
                );
            }
        }
        // Globs in flight go over everything on the tray.
        for o in self.oozes.iter().filter(|o| o.leap && t >= o.start) {
            let s = ((t - o.start) as f32 / o.len as f32).min(1.0);
            let (glob, shadow, trail) = slime::leap(o.from, o.to, s, o.seed);
            if let Some(b) = shadow {
                slime::shadow(f, q, b, 0.45);
            }
            slime::draw(f, q, &trail, o.side as usize);
            slime::draw(f, q, &glob, o.side as usize);
        }

        if let Some(sel) = self.selected {
            let (x, y) = center(sel);
            let c = if (self.t / 6).is_multiple_of(2) {
                art::WHITE
            } else {
                art::GLOW_FAR
            };
            hex_outline(f, x, y, art::R - 3.0, c, true);
        } else if let Some(h) = hover.filter(|_| self.phase == Phase::Player)
            && self.board.cells[h] == Cell::Mold(PLAYER)
        {
            let (x, y) = center(h);
            hex_outline(f, x, y, art::R - 3.0, art::WHITE, false);
        }

        // The labels that change; the rest are part of the wall.
        let (labels, q) = (&mut self.labels, &mut self.scene.quant);
        for side in [Side::Peni, Side::Rhizo] {
            let y = art::SIDE_Y[side as usize];
            f.blit(
                &self.scene.colonies[side as usize][0][STAGES - 1],
                art::HUD_X + 2,
                y + 12,
                Some(art::TRANSPARENT),
            );
            let score = self.board.count(side).to_string();
            label(labels, f, q, &score, 3, (art::HUD_X + 102, y + 36));
            let who = if side == PLAYER && !self.demo {
                "YOU"
            } else {
                "CPU"
            };
            label(labels, f, q, who, 2, (art::HUD_X + 102, y + 70));
        }
        for (k, line) in self.message.lines().enumerate() {
            let x = art::HUD_CX + if k % 2 == 0 { -2 } else { 3 };
            let y = art::MESSAGE_Y + k as i32 * art::MESSAGE_STEP;
            label(labels, f, q, line, 2, (x, y));
        }

        f.blit(&self.cursor, mouse.0, mouse.1, Some(art::TRANSPARENT));
    }
}

/// Draw the black label strip for `text`, centred on `at`, making it the first time.
fn label(
    labels: &mut HashMap<(String, usize), Rgba>,
    f: &mut Bitmap,
    q: &mut art::Quantizer,
    text: &str,
    scale: usize,
    at: (i32, i32),
) {
    let img = labels.entry((text.to_string(), scale)).or_insert_with(|| {
        let seed = text
            .bytes()
            .fold(17u32, |h, b| h.wrapping_mul(31) ^ b as u32);
        art::dymo::strip(text, art::dymo::BLACK, scale, seed)
    });
    let h = (img.px.len() / img.w) as i32;
    slime::blit(f, q, img, at.0 - img.w as i32 / 2, at.1 - h / 2);
}

fn hex_outline(f: &mut Bitmap, x: f32, y: f32, r: f32, c: u8, thick: bool) {
    let pts: Vec<(i32, i32)> = (0..=6)
        .map(|k| {
            let a = std::f32::consts::FRAC_PI_3 * k as f32 + std::f32::consts::FRAC_PI_6;
            ((x + r * a.cos()) as i32, (y + r * a.sin()) as i32)
        })
        .collect();
    for w in pts.windows(2) {
        f.line(w[0].0, w[0].1, w[1].0, w[1].1, c);
        if thick {
            f.line(w[0].0 + 1, w[0].1, w[1].0 + 1, w[1].1, c);
        }
    }
}

impl DosGame for Sporagon {
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
        let mut g = Self {
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
        g.new_game();
        Ok(g)
    }

    fn step(&mut self, dos: &mut Dos) {
        while let Some(k) = dos.getch() {
            match k.scan {
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
                let mut rng = Rng(self.rng.next() as u64 | 1);
                match board::best_move(&self.board, side, AI_DEPTH, || rng.next()) {
                    Some(mv) => self.play(side, mv),
                    None => self.begin_turn(side),
                }
            }
            Phase::Thinking(side, n) => self.phase = Phase::Thinking(side, n - 1),
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

const KEYS: &[(&str, &str)] = &[
    (
        "Mouse",
        "click your colony, then a cup: next to it clones, two away jumps",
    ),
    ("N", "new game"),
    ("D", "demo: the computer plays both sides"),
    ("Esc", "quit"),
];

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
