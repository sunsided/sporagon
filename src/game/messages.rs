//! What the black label strips on the wall say, one strip per line. Each line must fit the
//! label column, which the test below checks.

use crate::board::Side;

/// What the message strips say: the state of play, as the player should read it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Message {
    /// While the colonies of a move grow in.
    Growing,
    /// The human's turn.
    YourMove,
    /// The computer is about to move for this side (Penicillium only in demo mode).
    Spreads(Side),
    /// Game over, the human has more colonies.
    Win,
    /// Game over, the computer has more colonies.
    Lose,
    /// Demo game over, this side has more colonies.
    Wins(Side),
    /// Game over with equal counts.
    Draw,
}

impl Message {
    /// Every message, for the test that they fit the label column.
    #[cfg(test)]
    const ALL: [Message; 9] = [
        Message::Growing,
        Message::YourMove,
        Message::Spreads(Side::Peni),
        Message::Spreads(Side::Rhizo),
        Message::Win,
        Message::Lose,
        Message::Wins(Side::Peni),
        Message::Wins(Side::Rhizo),
        Message::Draw,
    ];

    /// The strips' text, one strip per line.
    pub(super) fn text(self) -> &'static str {
        match self {
            Message::Growing => "SOMETHING\nGROWS...",
            Message::YourMove => "YOUR MOVE\nPICK MOULD\nTHEN A CUP",
            Message::Spreads(Side::Peni) => "PENICILLIUM\nSPREADS...",
            Message::Spreads(Side::Rhizo) => "RHIZOPUS\nSPREADS...",
            Message::Win => "YOU WIN!\nTHE FRIDGE\nIS YOURS",
            Message::Lose => "YOU LOSE\nRHIZOPUS\nOWNS IT",
            Message::Wins(Side::Peni) => "PENICILLIUM\nWINS",
            Message::Wins(Side::Rhizo) => "RHIZOPUS\nWINS",
            Message::Draw => "A DRAW\nNOBODY\nCLEANS UP",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art;

    #[test]
    fn every_message_fits_the_label_column() {
        for line in Message::ALL.iter().flat_map(|m| m.text().lines()) {
            assert!(
                art::dymo::width(line, 2) <= art::HUD_W,
                "{line:?} is too long"
            );
        }
        for name in art::MOULDS
            .iter()
            .map(|m| &m.name)
            .chain([&art::KEYS, &art::GAME_TITLE])
        {
            assert!(
                art::dymo::width(name, 2) <= art::HUD_W,
                "{name:?} is too long"
            );
        }
    }
}
