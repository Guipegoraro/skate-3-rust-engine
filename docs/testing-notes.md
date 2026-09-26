# Testing notes

Running log of what works for testing gameplay changes without playing by hand.
Add to it whenever a test teaches something new.

## Scripted play harness (`crates/skate-game/src/tests/scripted_play.rs`)

- `Assets::load()` reads stock assets once; `Session::flat(&assets, |physics| ...)` or
  `Session::new(&assets, GamePhysics::load_with_map(root, Some(&map))?, ...)` builds one skater.
  The closure configures the physics before the skater loads (trainer, gravity, ...).
- `session.step(buttons, left_stick)` runs one full 60 Hz tick from a raw XInput state.
  Button constants: `A` (sprint on foot), `X` (jump), `Y` (step off / on the board).
- Read state straight from `session.skater` / `session.physics`, e.g. `root_position()`.
- Tests are `#[ignore]` because they need the private assets. Run:
  ```powershell
  $env:SKATE3_ASSET_ROOT="$PWD\assets"
  cargo test --locked -p skate-game --bin skate3rust -- --ignored --nocapture <test name>
  ```
  `SKATE3_ASSET_ROOT` must be absolute: the test's working directory is the crate folder.
- Asset loading is ~5 s per session, so several sessions per test are fine.

## Patterns

- **Compare against stock, not absolute numbers**: run the same script with the default and the
  modified setting and assert the ratio (SK-004: half gravity, jump 1.04 m → 1.93 m).
- **On foot from spawn**: tap `Y` at tick 20, then wait until ~tick 120 before scripting moves.
- **Map-specific bugs**: the flat test world is small; the skater runs off it within seconds.
  Load the real map with `SkateMap::load` and set `map.spawn` / `map.heading` to where the bug
  happened (SK-025). `SkateMap` is not `Clone`: mutate it in place between runs.
- **Blow-ups (NaN/inf)**: assert on the first lane that grows, not only on "finite", to find the
  feedback loop early (SK-025: the support velocity `w` lane).

## Lua mods without the game

- `crates/skate-mods/tests/runtime.rs` runs example mods headless: build a `Manager` on
  `sdk/examples`, set `m.snapshot` (fake player/map/actions), `dispatch` callbacks and read
  `m.commands` (overlay text, trainer, teleport...). SK-007 feeds a moving position and checks the
  overlay says 36 km/h. `examples_load_and_run` must list every example id.
- Package a mod for the game with `python tools/package_mod.py sdk/examples/<mod> mods/<mod>.zip`.

## Live game (BRP)

- The game serves Bevy Remote Protocol on port 15703 (`.local/play.cmd`). Only `Reflect`
  types are visible; `GamePhysics` is not, so physics checks belong in the scripted harness.
- Screenshots via BRP are the check for HUD/menu changes.
- Two instances (SK-002): `.local/play-2p.cmd` gives BRP 15703 (player 1) and 15704 (player 2); screenshot each port. Window rectangles can be read with Win32 `GetWindowRect` after `SetProcessDPIAware` (physical pixels). Launch detached (`Start-Process`) or the tool call waits on the games' output pipes.
- The first BRP connection right after launch can fail while the port is not yet listening; wait for `netstat` to show it LISTENING.
