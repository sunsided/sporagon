//! SPORAGON: Hexxagon in the back of a forgotten fridge. Your Penicillium against the
//! computer's Rhizopus, on a cracked egg tray, in VESA 640x480x256.

mod art;
mod board;
mod game;
mod host;

use game_toolkit_dosvideo::prelude::*;

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
    run::<game::Sporagon>(
        DosConfig::new("SPORAGON", Adapter::Svga { memory_kib: 1024 })
            .with_bus(BusTiming::PCI)
            .with_keys(KEYS)
            // In the browser: the canvas in index.html, or the one the games site creates.
            .with_canvas_id(host::CANVAS),
    )
}
