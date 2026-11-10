# SPORAGON

A Hexxagon-style board game in the back of a forgotten fridge. Your Penicillium fights the
computer's Rhizopus for a cracked egg tray, in 640x480 with 256 colors on an emulated VESA SVGA
card ([game-toolkit-dosvideo](https://github.com/sunsided/game-toolkit-dosvideo)).

![SPORAGON through the VGA monitor CRT filter: two mould colonies in each corner of a hexagonal egg tray, embossed label strips with names, scores and messages on a yellowed fridge wall](docs/screenshot.jpg)

## Playing

Play it in the browser at [games.markusmayer.dev](https://games.markusmayer.dev) (each release)
or [sunsided.github.io/sporagon](https://sunsided.github.io/sporagon/) (latest `main`), or natively:

```sh
task run        # or: cargo run --release
```

Each [release](https://github.com/sunsided/sporagon/releases) also has a Linux binary and the
browser build as a zip.

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

Code: `src/board.rs` (rules, AI), `src/game.rs` (state, turns, input, sound, game loop) with
`src/game/` (`play.rs` move animation, `draw.rs` the frame, `messages.rs` the label texts),
`src/art.rs` (layout and painting the scene) with `src/art/` (`spores.rs` the moulds, split
into `spores/penicillium.rs` and `spores/rhizopus.rs`; `slime.rs`; `fridge.rs` the background,
split into `fridge/tray.rs`, `wall.rs`, `fixtures.rs` and `drops.rs`; `dymo.rs` the labels;
`palette.rs` median cut and dithering; `noise.rs`; `shading.rs`).

```sh
task test       # rules, AI self-play, layout
task check      # fmt, clippy (native and wasm), tests
task web        # the browser build at http://localhost:8080 (needs trunk)
task screenshot # refresh docs/screenshot.jpg (needs a display)
```

### Releasing

`task release VERSION=x.y.z` ([cargo-release](https://github.com/crate-ci/cargo-release), set up
in `release.toml`; nothing goes to crates.io) bumps the version, turns `## [Unreleased]` in
`CHANGELOG.md` into the release's section, commits, tags `vX.Y.Z` and pushes. The tag runs
`.github/workflows/release.yml`: it builds the Linux binary and the browser zip, writes
`SHA256SUMS`, attests both builds' provenance and publishes the GitHub release with the
changelog section as notes. Then it announces the release to
[games.markusmayer.dev](https://github.com/sunsided/games.markusmayer.dev), which checks the
zip against the checksums and the attestation before it serves it.

The announcement runs in this repository's `games` environment, which only tags `v*` may
deploy to. It needs the repository variable `GAMES_ANNOUNCE_APP_ID` and the environment secret
`GAMES_ANNOUNCE_APP_PRIVATE_KEY`, the games site's announce app (see its README).

## License

EUPL-1.2

