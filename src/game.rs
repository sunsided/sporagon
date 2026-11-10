//! The game itself: its state, the turn flow, input, sound, the failing bulb, and the
//! [`DosGame`] loop that ties them together. Time is counted in frames (`Sporagon::t`, one per
//! [`DosGame::step`], at the display's refresh rate). Positions are screen pixels unless they
//! are cell indices into [`board::geometry`].

mod draw;
mod messages;
mod play;

use crate::art::{self, BRIDGE, Rgba, Rng, SPR, STAGES, Scene, slime};
use crate::board::{self, Board, Cell, Move, Side, geometry};
use game_toolkit_dosvideo::prelude::*;
use messages::Message;
use std::collections::HashMap;

/// The side the human plays, unless in demo mode.
const PLAYER: Side = Side::Peni;

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
pub struct Sporagon {
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
    message: Message,
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
        self.message = Message::Growing;
    }

    /// The cell under screen pixel `(x, y)`, if any.
    fn cell_at(&self, x: i32, y: i32) -> Option<usize> {
        let (q, r) = art::pixel_to_cell(x as f32, y as f32);
        board::index_of(q, r)
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
            self.message = Message::game_over(player_count, computer_count, self.demo);
            self.sfx = Some(Sfx {
                from: 200.0,
                to: 50.0,
                len: 40,
                t: 0,
                wobble: true,
            });
            self.phase = Phase::Over;
        } else if side == PLAYER && !self.demo {
            self.message = Message::YourMove;
            self.phase = Phase::Player;
        } else {
            self.message = Message::Spreads(side);
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
            message: Message::Growing,
        };
        game.new_game();
        // Painting the scene took the longest; the page can stop saying it is loading.
        crate::host::loaded();
        Ok(game)
    }

    /// One frame:
    ///
    /// 1. a restart from the games site starts over; keys: Esc quits, N starts over, D
    ///    toggles demo mode (handing the turn to the computer if the human was to move);
    /// 2. the clock ticks and finished animations, oozes and vanishings are dropped;
    /// 3. the phase advances: a click on the human's turn, the next turn once nothing is
    ///    moving on the tray any more, or the computer's move once its pause has run out;
    /// 4. sound, drawing and the copy to the screen on the vertical retrace;
    /// 5. the fluorescent tube, which now and then (1 in 500 per frame) plays a blink pattern
    ///    by dimming the palette.
    fn step(&mut self, dos: &mut Dos) {
        // The games site opened the game again: start it fresh.
        if crate::host::restart_requested() {
            self.new_game();
        }
        while let Some(key) = dos.getch() {
            match key.scan {
                sc::ESC => dos.exit(),
                sc::N => self.new_game(),
                sc::D => {
                    self.demo = !self.demo;
                    // Whenever Penicillium is about to move, hand it over at once: to the
                    // computer, or back to the human during the computer's pause.
                    let peni_to_move = match self.phase {
                        Phase::Player => true,
                        Phase::Thinking(side, _) => side == PLAYER,
                        _ => false,
                    };
                    if peni_to_move {
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
