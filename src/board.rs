//! Hexxagon rules on a hexagonal board of radius 4 (61 cells, 3 knocked out): a move to a
//! neighbouring cell clones the piece, a move two cells away jumps (the source empties). Either
//! way every enemy piece around the target is converted.

use std::sync::OnceLock;

pub const RADIUS: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// The player: blue-green Penicillium.
    Peni,
    /// The computer: black bread mould, Rhizopus.
    Rhizo,
}

impl Side {
    pub fn other(self) -> Self {
        match self {
            Side::Peni => Side::Rhizo,
            Side::Rhizo => Side::Peni,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    Empty,
    Hole,
    Mold(Side),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    pub from: usize,
    pub to: usize,
    pub jump: bool,
}

/// Cell coordinates (axial `q, r`) and precomputed neighbourhoods.
pub struct Geometry {
    pub coords: Vec<(i32, i32)>,
    pub near: Vec<Vec<usize>>,
    pub far: Vec<Vec<usize>>,
}

pub fn dist(a: (i32, i32), b: (i32, i32)) -> i32 {
    let (dq, dr) = (a.0 - b.0, a.1 - b.1);
    (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
}

pub fn geometry() -> &'static Geometry {
    static G: OnceLock<Geometry> = OnceLock::new();
    G.get_or_init(|| {
        let mut coords = Vec::new();
        for r in -RADIUS..=RADIUS {
            for q in -RADIUS..=RADIUS {
                if dist((q, r), (0, 0)) <= RADIUS {
                    coords.push((q, r));
                }
            }
        }
        let ring = |d: i32| -> Vec<Vec<usize>> {
            coords
                .iter()
                .map(|&a| {
                    (0..coords.len())
                        .filter(|&j| dist(a, coords[j]) == d)
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

pub fn index_of(q: i32, r: i32) -> Option<usize> {
    geometry().coords.iter().position(|&c| c == (q, r))
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Board {
    pub cells: Vec<Cell>,
}

const DIRS: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

impl Board {
    /// The classic opening: pieces in the six corners, alternating, and three holes around
    /// the centre.
    pub fn new() -> Self {
        let g = geometry();
        let mut cells = vec![Cell::Empty; g.coords.len()];
        for (k, (dq, dr)) in DIRS.iter().enumerate() {
            let side = if k % 2 == 0 { Side::Peni } else { Side::Rhizo };
            cells[index_of(dq * RADIUS, dr * RADIUS).unwrap()] = Cell::Mold(side);
            if k % 2 == 1 {
                cells[index_of(*dq, *dr).unwrap()] = Cell::Hole;
            }
        }
        Self { cells }
    }

    pub fn count(&self, side: Side) -> usize {
        self.cells
            .iter()
            .filter(|&&c| c == Cell::Mold(side))
            .count()
    }

    pub fn moves(&self, side: Side) -> Vec<Move> {
        let g = geometry();
        let mut out = Vec::new();
        for (to, &c) in self.cells.iter().enumerate() {
            if c != Cell::Empty {
                continue;
            }
            // One clone per target is enough: where it was cloned from does not matter.
            if let Some(&from) = g.near[to]
                .iter()
                .find(|&&j| self.cells[j] == Cell::Mold(side))
            {
                out.push(Move {
                    from,
                    to,
                    jump: false,
                });
            }
            for &from in &g.far[to] {
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

    pub fn has_moves(&self, side: Side) -> bool {
        !self.moves(side).is_empty()
    }

    /// Whether `from -> to` is legal for `side`, and if so as which move.
    pub fn legal(&self, side: Side, from: usize, to: usize) -> Option<Move> {
        if self.cells[from] != Cell::Mold(side) || self.cells[to] != Cell::Empty {
            return None;
        }
        let g = geometry();
        match dist(g.coords[from], g.coords[to]) {
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

    /// Play `mv` for `side`; returns the converted cells.
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

    /// When `side` cannot move, the opponent overgrows every empty cell. Returns them.
    pub fn fill_for(&mut self, side: Side) -> Vec<usize> {
        let empty: Vec<usize> = (0..self.cells.len())
            .filter(|&i| self.cells[i] == Cell::Empty)
            .collect();
        for &i in &empty {
            self.cells[i] = Cell::Mold(side);
        }
        empty
    }

    fn score(&self, side: Side) -> i32 {
        self.count(side) as i32 - self.count(side.other()) as i32
    }
}

/// Negamax with alpha-beta; `side` is to move.
fn search(b: &Board, side: Side, depth: u32, mut alpha: i32, beta: i32) -> i32 {
    if depth == 0 {
        return b.score(side);
    }
    let mut moves = b.moves(side);
    if moves.is_empty() {
        // Game over: the opponent takes the rest of the board.
        let mut end = b.clone();
        end.fill_for(side.other());
        return end.score(side) * 100;
    }
    order(b, side, &mut moves);
    let mut best = i32::MIN + 1;
    for mv in moves {
        let mut next = b.clone();
        next.apply(side, mv);
        let v = -search(&next, side.other(), depth - 1, -beta, -alpha);
        best = best.max(v);
        alpha = alpha.max(v);
        if alpha >= beta {
            break;
        }
    }
    best
}

/// Best first: most captures, clones before jumps.
fn order(b: &Board, side: Side, moves: &mut [Move]) {
    let g = geometry();
    moves.sort_by_cached_key(|m| {
        let caps = g.near[m.to]
            .iter()
            .filter(|&&j| b.cells[j] == Cell::Mold(side.other()))
            .count() as i32;
        -(caps * 2 + i32::from(!m.jump))
    });
}

/// The computer's move. `noise` breaks ties between equally good moves.
pub fn best_move(
    b: &Board,
    side: Side,
    depth: u32,
    mut noise: impl FnMut() -> u32,
) -> Option<Move> {
    let mut moves = b.moves(side);
    order(b, side, &mut moves);
    let mut best: Option<(i32, u32, Move)> = None;
    for mv in moves {
        let mut next = b.clone();
        next.apply(side, mv);
        let v = -search(&next, side.other(), depth - 1, -i32::MAX, i32::MAX);
        let tie = noise();
        if best.is_none_or(|(bv, bt, _)| v > bv || (v == bv && tie > bt)) {
            best = Some((v, tie, mv));
        }
    }
    best.map(|(_, _, m)| m)
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
