//! Putting a move on screen: the pseudopod or leaping glob, the new colony growing in, the old
//! one shrinking away, and every takeover creeping into its cup.

use super::*;

impl Sporagon {
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
    pub(super) fn play(&mut self, side: Side, mv: Move) {
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
}
