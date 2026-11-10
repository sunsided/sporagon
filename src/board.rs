//! Hexxagon rules on a hexagonal board of radius 4 (61 cells, 3 knocked out): a move to a
//! neighbouring cell clones the piece, a move two cells away jumps (the source empties). Either
//! way every enemy piece around the target is converted.
//!
//! Cells are addressed by their index into [`Geometry::coords`]; the board itself is just one
//! [`Cell`] per index.

use std::sync::OnceLock;

/// Cells from the centre to the edge of the board; 4 gives 61 cells.
pub const RADIUS: i32 = 4;

/// One of the two players.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// The player: blue-green Penicillium.
    Peni,
    /// The computer: black bread mould, Rhizopus.
    Rhizo,
}

impl Side {
    /// The opponent.
    pub fn other(self) -> Self {
        match self {
            Side::Peni => Side::Rhizo,
            Side::Rhizo => Side::Peni,
        }
    }
}

/// What occupies a cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    /// A free cup that either side can move into.
    Empty,
    /// Knocked out: never played on, and blocks nothing (jumps go over it).
    Hole,
    /// A colony of that side.
    Mold(Side),
}

/// A move, as cell indices.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    /// The colony that moves or clones.
    pub from: usize,
    /// The empty cell it moves into.
    pub to: usize,
    /// `true` for a jump two cells away (`from` empties), `false` for a clone to a neighbour.
    pub jump: bool,
}

/// Cell coordinates (axial `q, r`) and precomputed neighbourhoods.
pub struct Geometry {
    /// Axial `(q, r)` of every cell, by cell index: row by row in `r`, then `q`, ascending.
    pub coords: Vec<(i32, i32)>,
    /// Per cell, the indices of the up to 6 cells at distance 1 (clone targets), ascending.
    pub near: Vec<Vec<usize>>,
    /// Per cell, the indices of the up to 12 cells at distance 2 (jump targets), ascending.
    pub far: Vec<Vec<usize>>,
}

/// Hex distance between two axial coordinates: the number of steps from cell to cell.
pub fn dist(a: (i32, i32), b: (i32, i32)) -> i32 {
    let (dq, dr) = (a.0 - b.0, a.1 - b.1);
    // Cube distance with the third coordinate `s = -q - r`.
    (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
}

/// The board layout, built on first use and shared from then on.
pub fn geometry() -> &'static Geometry {
    static GEOMETRY: OnceLock<Geometry> = OnceLock::new();

    GEOMETRY.get_or_init(|| {
        let mut coords = Vec::new();
        for r in -RADIUS..=RADIUS {
            for q in -RADIUS..=RADIUS {
                if dist((q, r), (0, 0)) <= RADIUS {
                    coords.push((q, r));
                }
            }
        }

        // For every cell, the indices of all cells exactly `distance` away.
        let ring = |distance: i32| -> Vec<Vec<usize>> {
            coords
                .iter()
                .map(|&cell| {
                    (0..coords.len())
                        .filter(|&j| dist(cell, coords[j]) == distance)
                        .collect()
                })
                .collect()
        };

        Geometry {
            near: ring(1),
            far: ring(2),
            coords,
        }
    })
}

/// The index of the cell at axial `(q, r)`, or `None` off the board.
pub fn index_of(q: i32, r: i32) -> Option<usize> {
    geometry().coords.iter().position(|&c| c == (q, r))
}

/// A position: what is in each cell.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Board {
    /// One entry per cell index of [`geometry`].
    pub cells: Vec<Cell>,
}

/// The six axial neighbour directions; scaled by [`RADIUS`] they reach the corners.
const DIRS: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

impl Board {
    /// The classic opening: pieces in the six corners, alternating, and three holes around
    /// the centre.
    pub fn new() -> Self {
        let geo = geometry();
        let mut cells = vec![Cell::Empty; geo.coords.len()];

        for (i, (dq, dr)) in DIRS.iter().enumerate() {
            let side = if i % 2 == 0 { Side::Peni } else { Side::Rhizo };
            cells[index_of(dq * RADIUS, dr * RADIUS).unwrap()] = Cell::Mold(side);
            // The holes sit next to the centre, on the Rhizopus directions.
            if i % 2 == 1 {
                cells[index_of(*dq, *dr).unwrap()] = Cell::Hole;
            }
        }

        Self { cells }
    }

    /// How many colonies `side` has.
    pub fn count(&self, side: Side) -> usize {
        self.cells
            .iter()
            .filter(|&&c| c == Cell::Mold(side))
            .count()
    }

    /// Every distinct move for `side`, by target cell ascending.
    ///
    /// For each empty target: at most one clone (from its first own neighbour, since any
    /// source gives the same result), then one jump per own colony two away, in [`Geometry::far`]
    /// order. The search's move ordering and tie-breaking build on this order.
    pub fn moves(&self, side: Side) -> Vec<Move> {
        let geo = geometry();
        let mut out = Vec::new();

        for (to, &c) in self.cells.iter().enumerate() {
            if c != Cell::Empty {
                continue;
            }

            // One clone per target is enough: where it was cloned from does not matter.
            if let Some(&from) = geo.near[to]
                .iter()
                .find(|&&j| self.cells[j] == Cell::Mold(side))
            {
                out.push(Move {
                    from,
                    to,
                    jump: false,
                });
            }

            for &from in &geo.far[to] {
                if self.cells[from] == Cell::Mold(side) {
                    out.push(Move {
                        from,
                        to,
                        jump: true,
                    });
                }
            }
        }

        out
    }

    /// Whether `side` has any move at all.
    pub fn has_moves(&self, side: Side) -> bool {
        !self.moves(side).is_empty()
    }

    /// Whether `from -> to` is legal for `side`, and if so as which move.
    pub fn legal(&self, side: Side, from: usize, to: usize) -> Option<Move> {
        if self.cells[from] != Cell::Mold(side) || self.cells[to] != Cell::Empty {
            return None;
        }

        let geo = geometry();
        match dist(geo.coords[from], geo.coords[to]) {
            1 => Some(Move {
                from,
                to,
                jump: false,
            }),
            2 => Some(Move {
                from,
                to,
                jump: true,
            }),
            _ => None,
        }
    }

    /// Play `mv` for `side`, which the caller has checked is legal; returns the converted
    /// cells, in [`Geometry::near`] order.
    ///
    /// A jump empties `from`; either way `to` becomes `side`'s, and so does every enemy
    /// colony next to `to`.
    pub fn apply(&mut self, side: Side, mv: Move) -> Vec<usize> {
        if mv.jump {
            self.cells[mv.from] = Cell::Empty;
        }
        self.cells[mv.to] = Cell::Mold(side);

        let converted: Vec<usize> = geometry().near[mv.to]
            .iter()
            .copied()
            .filter(|&j| self.cells[j] == Cell::Mold(side.other()))
            .collect();
        for &j in &converted {
            self.cells[j] = Cell::Mold(side);
        }

        converted
    }

    /// When `side` cannot move, the opponent overgrows every empty cell. Returns them,
    /// ascending.
    pub fn fill_for(&mut self, side: Side) -> Vec<usize> {
        let empty: Vec<usize> = (0..self.cells.len())
            .filter(|&i| self.cells[i] == Cell::Empty)
            .collect();
        for &i in &empty {
            self.cells[i] = Cell::Mold(side);
        }
        empty
    }

    /// Material from `side`'s point of view: own colonies minus the opponent's.
    fn score(&self, side: Side) -> i32 {
        self.count(side) as i32 - self.count(side.other()) as i32
    }
}

/// Negamax with alpha-beta; `side` is to move. Returns the value of `board` for `side`,
/// looking `depth` plies ahead, within the window `alpha..beta`.
///
/// The value is [`Board::score`] at the horizon. A side without moves ends the game: the
/// opponent takes every empty cell, and the final score counts 100 times over, so a won or
/// lost game outweighs any material swing. Each child is scored as minus its value for the
/// opponent, with the window negated and swapped; once `alpha >= beta` the remaining moves
/// cannot matter and are cut. The result is exact inside the window and only a bound outside
/// it, which is why good moves first ([`order`]) prunes more.
fn search(board: &Board, side: Side, depth: u32, mut alpha: i32, beta: i32) -> i32 {
    if depth == 0 {
        return board.score(side);
    }

    let mut moves = board.moves(side);
    if moves.is_empty() {
        // Game over: the opponent takes the rest of the board.
        let mut end = board.clone();
        end.fill_for(side.other());
        return end.score(side) * 100;
    }

    order(board, side, &mut moves);
    // `i32::MIN + 1`, so negating it cannot overflow.
    let mut best = i32::MIN + 1;
    for mv in moves {
        let mut next = board.clone();
        next.apply(side, mv);
        let value = -search(&next, side.other(), depth - 1, -beta, -alpha);

        best = best.max(value);
        alpha = alpha.max(value);
        if alpha >= beta {
            break;
        }
    }

    best
}

/// Best first: most captures, clones before jumps.
///
/// The key is `2 * captures + (1 for a clone)`, so one capture outweighs the clone bonus. The
/// sort is stable: equal keys keep the [`Board::moves`] order.
fn order(board: &Board, side: Side, moves: &mut [Move]) {
    let geo = geometry();
    moves.sort_by_cached_key(|mv| {
        let captures = geo.near[mv.to]
            .iter()
            .filter(|&&j| board.cells[j] == Cell::Mold(side.other()))
            .count() as i32;
        -(captures * 2 + i32::from(!mv.jump))
    });
}

/// The computer's move for `side`, searched `depth` plies deep (at least 1), or `None` if it
/// has none.
///
/// Every root move gets its own full-window [`search`], so each value is exact and equal
/// moves really are equal. `noise` breaks ties between equally good moves: it is called once
/// per root move, in [`order`], and the highest draw among the best values wins.
pub fn best_move(
    board: &Board,
    side: Side,
    depth: u32,
    mut noise: impl FnMut() -> u32,
) -> Option<Move> {
    let mut moves = board.moves(side);
    order(board, side, &mut moves);

    // `(value, tie-break draw, move)` of the best so far.
    let mut best: Option<(i32, u32, Move)> = None;
    for mv in moves {
        let mut next = board.clone();
        next.apply(side, mv);
        let value = -search(&next, side.other(), depth - 1, -i32::MAX, i32::MAX);

        let tie = noise();
        let is_better = |(best_value, best_tie, _): (i32, u32, Move)| {
            value > best_value || (value == best_value && tie > best_tie)
        };
        if best.is_none_or(is_better) {
            best = Some((value, tie, mv));
        }
    }

    best.map(|(_, _, mv)| mv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening() {
        let b = Board::new();
        assert_eq!(b.cells.len(), 61);
        assert_eq!(b.count(Side::Peni), 3);
        assert_eq!(b.count(Side::Rhizo), 3);
        assert_eq!(b.cells.iter().filter(|&&c| c == Cell::Hole).count(), 3);
        // A corner has 3 neighbours and some cells two away.
        assert!(b.moves(Side::Peni).iter().any(|m| !m.jump));
        assert!(b.moves(Side::Peni).iter().any(|m| m.jump));
    }

    #[test]
    fn clone_jump_and_convert() {
        let mut b = Board::new();
        let a = index_of(0, 0).unwrap();
        let e1 = index_of(1, 0).unwrap();
        let e2 = index_of(0, 1).unwrap();
        b.cells[e1] = Cell::Mold(Side::Rhizo);
        b.cells[e2] = Cell::Mold(Side::Rhizo);
        let from = index_of(-1, 0).unwrap();
        b.cells[from] = Cell::Mold(Side::Peni);
        b.cells[a] = Cell::Empty;
        let mv = b.legal(Side::Peni, from, a).unwrap();
        assert!(!mv.jump);
        let conv = b.apply(Side::Peni, mv);
        assert_eq!(conv.len(), 2);
        assert_eq!(b.cells[from], Cell::Mold(Side::Peni));
        assert_eq!(b.cells[e1], Cell::Mold(Side::Peni));

        let far = index_of(-3, 0).unwrap();
        let mv = b.legal(Side::Peni, from, far).unwrap();
        assert!(mv.jump);
        b.apply(Side::Peni, mv);
        assert_eq!(b.cells[from], Cell::Empty);
        assert!(b.legal(Side::Peni, a, index_of(4, -4).unwrap()).is_none());
    }

    #[test]
    fn ai_takes_the_capture() {
        let mut b = Board::new();
        for c in &mut b.cells {
            if *c != Cell::Hole {
                *c = Cell::Empty;
            }
        }
        b.cells[index_of(-2, 0).unwrap()] = Cell::Mold(Side::Rhizo);
        for (q, r) in [(0, -1), (1, -1)] {
            b.cells[index_of(q, r).unwrap()] = Cell::Mold(Side::Peni);
        }
        let mut n = 0;
        let mv = best_move(&b, Side::Rhizo, 3, || {
            n += 1;
            n
        })
        .unwrap();
        let mut after = b.clone();
        assert_eq!(after.apply(Side::Rhizo, mv).len(), 2);
    }

    #[test]
    fn self_play_ends() {
        let mut b = Board::new();
        let mut side = Side::Peni;
        let mut n = 7u32;
        let mut noise = || {
            n = n.wrapping_mul(1_103_515_245).wrapping_add(12345);
            n
        };
        for _ in 0..500 {
            let Some(mv) = best_move(&b, side, 3, &mut noise) else {
                b.fill_for(side.other());
                break;
            };
            b.apply(side, mv);
            side = side.other();
        }
        assert!(
            !b.cells.contains(&Cell::Empty)
                || b.count(Side::Peni) == 0
                || b.count(Side::Rhizo) == 0
        );
    }
}
