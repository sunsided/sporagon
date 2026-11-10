# SPORAGON

A Hexxagon-style board game in the back of a forgotten fridge. Your Penicillium fights the
computer's Rhizopus for a cracked egg tray, in 640x480 with 256 colors on an emulated VESA SVGA
card ([game-toolkit-dosvideo](https://github.com/sunsided/game-toolkit-dosvideo)).

![SPORAGON through the VGA monitor CRT filter: two mould colonies in each corner of a hexagonal egg tray, embossed label strips with names, scores and messages on a yellowed fridge wall](docs/screenshot.jpg)

## Playing

```sh
task run        # or: cargo run --release
```

Click one of your colonies, then an empty cup:

- a cup **next to it**: the colony spreads and you keep the original.
- a cup **two away**: the colony jumps there and leaves its old cup.

Either way, every Rhizopus colony touching the target cup is overgrown and turns into yours.
If a side cannot move, the other side grows over every empty cup. The side with more colonies
when the tray is full wins.

| Key | |
|-----|---|
| Mouse | select and move |
| `N` | new game |
| `D` | demo: the computer plays both sides |
| `Esc` | quit |
| `Alt+C` | cycle the CRT filter |
| `Alt+Enter` | fullscreen |

## How it's made

All graphics are generated at startup, no image files. Everything is painted in true color
(fridge wall, egg tray, ten growth stages of four variants per mould), then reduced to one
256-color palette by median cut and dithered with a 4x4 Bayer matrix, the way scanned art was
prepared for VGA. Some details:

- **Penicillium**: velvety blue-green zones, radial furrows, dusty spore bloom, a white fuzzy
  margin of young hyphae, and amber guttation droplets on mature colonies.
- **Rhizopus**: a tangle of white cottony hyphae with sporangia that ripen from white to black.
- **Slime**: every colony sits in a glossy mucus puddle (metaballs, shaded and blended
  translucently into the 256-color frame). Neighbouring colonies of one kind are joined by
  slime and grow across the ridge between their cups into one mould mass.
- **Moves**: a clone pushes a pulsing pseudopod into the next cup before the new colony
  grows; a jump gathers a glob that snaps off its thread and arcs over the tray with a shadow
  and a trail of drops while the old colony shrinks away; every takeover is a pseudopod
  reaching into the victim's cup.
- **Fridge**: a dimpled egg tray lit by the bulb, condensation drops, brown drips, black
  mildew in the corners, and a bulb that flickers through the palette now and then.
- **Labels**: embossed label-maker strips stuck crooked on the wall, each with its shadow; a
  hand-drawn 5x7 font punched into coloured plastic, shown at two or three times its size.

The frame is composed in system memory and copied to the linear framebuffer each vertical
retrace. Moves and takeovers play PC-speaker sweeps. The computer searches three moves deep
with alpha-beta pruning.

## Development

Code: `src/board.rs` (rules, AI), `src/main.rs` (game loop, animation, screen),
`src/art.rs` (palette and layout) with `src/art/` (`spores.rs` the moulds, `slime.rs`, `fridge.rs`,
`dymo.rs` the labels).

```sh
task test       # rules, AI self-play, layout
task check      # fmt, clippy, tests
task screenshot # refresh docs/screenshot.jpg (needs a display)
```

## License

EUPL-1.2

