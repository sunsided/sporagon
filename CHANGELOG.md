# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-11-10

### Fixed

- Quick clicks and trackpad taps that start and end within one frame now select and move
  colonies; they were dropped before.
- The browser build no longer asks for an `assets/manifest.txt` on start, a 404 on every page
  that embeds it. Both fixes come with game-toolkit-dosvideo 0.3.2.

## [0.1.0] - 2026-11-10

### Added

- SPORAGON: Hexxagon-style rules on a 61-cup egg tray with three knocked-out cups. Clone to a
  neighbouring cup or jump two away, and take over every enemy colony around the target.
- A computer opponent that searches three moves deep with alpha-beta pruning, and a demo mode
  (`D`) in which it plays both sides.
- Procedurally painted art in 640x480x256 on an emulated VESA SVGA card: Penicillium and
  Rhizopus colonies in ten growth stages, glossy slime puddles, bridges and necks that merge
  neighbours into one mould mass, pseudopods and leaping globs for every move, and a rundown
  fridge with a flickering fluorescent tube.
- Embossed label-maker strips for the HUD, PC-speaker sound effects.
- A browser build (Trunk, WebAssembly) alongside the native one, released to
  [games.markusmayer.dev](https://games.markusmayer.dev).

[Unreleased]: https://github.com/sunsided/sporagon/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/sunsided/sporagon/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/sunsided/sporagon/releases/tag/v0.1.0
