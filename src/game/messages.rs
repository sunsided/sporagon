//! What the black label strips on the wall say, one strip per line. Each line must fit the
//! label column, which the test below checks.

/// While the colonies of a move grow in.
pub(super) const MSG_GROWING: &str = "SOMETHING\nGROWS...";
/// The human's turn.
pub(super) const MSG_YOUR_MOVE: &str = "YOUR MOVE\nPICK MOULD\nTHEN A CUP";
/// The computer is about to move for Penicillium (demo mode).
pub(super) const MSG_PENI_THINKS: &str = "PENICILLIUM\nSPREADS...";
/// The computer is about to move for Rhizopus.
pub(super) const MSG_RHIZO_THINKS: &str = "RHIZOPUS\nSPREADS...";
/// Game over, the human has more colonies.
pub(super) const MSG_WIN: &str = "YOU WIN!\nTHE FRIDGE\nIS YOURS";
/// Game over, the computer has more colonies.
pub(super) const MSG_LOSE: &str = "YOU LOSE\nRHIZOPUS\nOWNS IT";
/// Demo game over, Penicillium has more colonies.
pub(super) const MSG_PENI_WINS: &str = "PENICILLIUM\nWINS";
/// Demo game over, Rhizopus has more colonies.
pub(super) const MSG_RHIZO_WINS: &str = "RHIZOPUS\nWINS";
/// Game over with equal counts.
pub(super) const MSG_DRAW: &str = "A DRAW\nNOBODY\nCLEANS UP";
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art;

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
