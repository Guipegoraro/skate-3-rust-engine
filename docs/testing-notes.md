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

- **Tricks from the stock patterns**: load the `.pat` file (`skate_data::gesture_patterns::load`), walk the
  pattern's key points with `step_sticks` (`stick_from_pattern` converts to XInput), then read
  `session.controls.recognized_gesture()` (SK-027). The harness calls `publish_gestures` like the
  game's `controls::sample` system; without it no trick is ever recognized.

- **Compare against stock, not absolute numbers**: run the same script with the default and the
  modified setting and assert the ratio (SK-004: half gravity, jump 1.04 m → 1.93 m).
- **On foot from spawn**: tap `Y` at tick 20, then wait until ~tick 120 before scripting moves.
- **Map-specific bugs**: the flat test world is small; the skater runs off it within seconds.
  Load the real map with `SkateMap::load` and set `map.spawn` / `map.heading` to where the bug
  happened (SK-025). `SkateMap` is not `Clone`: mutate it in place between runs.
- **Test course at any spot** (SK-062): `GamePhysics::load_course_at(root, Difficulty::Normal, spawn, heading)`
  gives the default world (rails, ramps, halfpipe) with the board at `spawn` (wheel-ground anchor),
  heading about +Y with 0 = +Z. `GamePhysics::load` is the *flat* world with no rails. Set a riding
  speed after ~40 settle ticks with `impulse::queue` (SK-005). Deck frame for probes:
  `solve::deck_frame(&physics.board)` = [right, up, forward, position].
- **Ollie timing**: the SK-031 flick (right stick down 8 ticks, up 6) pops ~14 ticks after the
  flick; the deck apex is ~0.62 m and it lands ~45 ticks after the flick at 3-7 m/s. Calibrate the
  flick distance with one ollie on open floor instead of guessing (`grind_snap.rs` `calibrate`).
- **Why a gate rejects**: temporary `eprintln!` at each `return None` of the native gate, run the
  quick subset, then remove them; the sweep plus prints took ~2 min (SK-062 found the 11 deg angle gate).
- **Blow-ups (NaN/inf)**: assert on the first lane that grows, not only on "finite", to find the
  feedback loop early (SK-025: the support velocity `w` lane).

## Lua mods without the game

- `crates/skate-mods/tests/runtime.rs` runs example mods headless: build a `Manager` on
  `sdk/examples`, set `m.snapshot` (fake player/map/actions), `dispatch` callbacks and read
  `m.commands` (overlay text, trainer, teleport...). SK-007 feeds a moving position and checks the
  overlay says 36 km/h. `examples_load_and_run` must list every example id.
- Package a mod for the game with `python tools/package_mod.py sdk/examples/<mod> mods/<mod>.zip`.

## Live game tests (`scripts/GameTest.psm1`)

Use this module for anything that needs the running game; it replaces hand-written launch,
sleep and screenshot snippets. Default map is the **test world** (`--test-world`), which
loads in seconds; pass `-Map` only when the map matters.

```powershell
Import-Module .\scripts\GameTest.psm1 -Force
$g = Start-TestGame -Stage -Mods @{'guipegoraro.skater-size' = @{size = 2}} -Model stock `
     -Env @{SKATE_FORCE_PHYSICS_FAILURE = '25'}
Save-GameScreenshot $g 'size-2'      # logs/test-shots/size-2.png, then Read it
Send-GameKeys $g @('KeyW') 1000      # keyboard input through BRP
Set-GameWindow $g Minimize           # throws if Windows did not minimise it
Get-GameIssues $g                    # non-finite / CRASH_RECOVERY / REPORT_PANIC / ERROR lines
Test-GameAlive $g
Stop-TestGame $g                     # closes the game, restores mod and model settings
```

- `Start-TestGame` kills any running game, optionally stages `target/` into `bin/` (`-Stage`),
  backs up every settings file it touches (mod settings, custom model selection) and
  picks a free BRP port from 15705 up (iw4L often owns 15702/15703). It waits for BRP to
  answer, then 8 s for the world to settle. Logs: `logs/test-<time>.log`/`.stderr.log`.
- Scenario scripts wrap it and exit 1 on failure, so they work as regression checks:
  `scripts/Test-Minimize.ps1` (minimise must not fail the camera or panic, SK-037) and
  `scripts/Test-MinimizedRecovery.ps1` (a physics failure while minimised recovers after restore).
  Add one per live bug; first run it on the old exe and see it go red.
- The exe is a crash-report supervisor plus a child. Both own "visible" helper windows (the
  supervisor's `PseudoConsoleWindow`, winit's `Winit Thread Event Target`), so the game window is
  found by its title "Skate 3 Rust Engine" (`EnumWindows`). Minimising a helper instead looked
  exactly like a Bevy bug (window restored at 0x0): check sizes with `Get-GameWindowSize`
  (Windows client area vs Bevy resolution) before blaming the engine.
- `Start-TestGame` takes a lock (`logs/game-test.lock`), so parallel sessions/agents wait for each
  other instead of killing each other's game; `-Stage` throws if `bin/` is in use. Always
  `Stop-TestGame` in a `finally`.
- `Start-TestGame` also stops `skate-steam-relay`, which can outlive the game and hold `bin/steam-relay` files so staging fails. The module runs on Windows PowerShell 5 too (no `utf8NoBOM` there; `Write-Utf8` writes BOM-less UTF-8).
- `scripts/Test-SkaterSize.ps1`: screenshots per mod setting; the pattern for any visual option (one Start/Stop per value).
- `Start-TestGame` backs up `graphics.json` and `game-options.json` too: a test that changes options through the menu once left the player's settings with SSAO/bloom on.
- When the monitors are asleep (user away), the game cannot create its window surface and crashes at startup: live tests are impossible then; do headless work (scripted harness, unit tests) instead.
- Screenshot size is a cheap sanity check: a 1x1 PNG means the game rendered into an empty window.
- Only `Reflect` types are visible through BRP; `GamePhysics` is not, so physics checks belong
  in the scripted harness.
- Two instances (SK-002): `.local/play-2p.cmd` gives BRP 15703 (player 1) and 15704 (player 2).
  Window rectangles: Win32 `GetWindowRect` after `SetProcessDPIAware` (physical pixels).
- Crash recovery (SK-033): `-Env @{SKATE_FORCE_PHYSICS_FAILURE='30'}` and look for
  `CRASH_RECOVERY ... restored to` in `Get-GameIssues`. Panics are `REPORT_PANIC <message>`.
- Custom models: import without the picker with `python tools/setup.py --character-import
  --library-import <library> --reference assets/private/skater.glb --result r.json
  --noninteractive <model.glb>`, then `Start-TestGame -Model <id>`.
- Finding entities by name (SK-034): BRP `world.query` with `bevy_ecs::name::Name` and
  `Transform`, filtered on the name in PowerShell; `scripts/Test-Pedestrians.ps1` reads NPC
  roots twice 3 s apart to check they walk.
- Animation checks from screenshots: take 3-4 shots ~0.3 s apart and crop/upscale the character
  with .NET `System.Drawing` (the global Python has no Pillow); legs must change pose.
- Input latency (SK-060): `-Env @{SKATE_LATENCY_PROBE='1'}` logs `latency probe: buttons ...
  polled frame -> physics tick -> first drawn frame -> fully drawn frame` per button press
  (grep the log). `SKATE_VIDEO_FX=smoothing=0` and `SKATE_FPS_LIMIT=60|90|120` give A/B runs
  without touching graphics.json. Screen time is ~1 frame after "fully drawn" (pipelined
  render), not in the log.
- FPS A/B: `SKATE_PERF_REPORT=<file.json>` makes the game record 10-25 s after start, write the
  report and exit. Test world: ~88 FPS, 11.4 ms median on this machine (SK-034, 0 vs 10 NPCs).

## Python pipeline

- The asset pipeline needs numpy/Pillow: `python -m venv .local/venv` then `.local/venv/Scripts/python -m pip install -r tools/requirements-setup.txt`. Run pipeline tests with that interpreter; with the global Python, `test_optional_content` fails on imports, which is not a real failure.
- Converting a map validates it by launching the game with `--check-assets`; close the game first.

## Original game as reference

- `scripts/Run-Skate3RecompTrace.ps1` runs Skate3Recomp with an XMA-tracing runtime (tools/recomp_trace); `tools/audio_trace_match.py` turns the log into `<bank>/<index>` per played buffer. First run matched the menu sounds to `sk8_menu/53`/`54`. Music/ambience show as unknown (streamed, not in audiofiles.big).
- Skate3Recomp can be driven without a player (SK-032): `Run-Skate3RecompTrace.ps1 -Automate` reaches the plaza in ~20 s via the recomp's `--skate3_demo_path=true`, and the patched runtime reads the pad from `logs/pad-script.txt` (needs `--mnk_mode=true` so a pad is connected; it answers unfocused). Put markers in the script and use `audio_trace_match.py --recurring <action> <s>`: the plaza is full of pedestrians and NPC skaters whose footsteps, rolls and pops land in the same trace, so trust only ids that follow every repetition (5 ollies were enough).
- Original sound timing: an input's sounds start ~0.3-0.5 s after the pad step (pop ~0.3-0.5 s after the flick, landing ~1 s). The original stacks several samples per event (pop: 4-6 Skate_Collisions voices plus an Sk8_Air_Flip_Tricks whoosh; menu move: sk8_menu/53 and /54 together), hence `layers` in events.json.
- Bail by script (SK-042): the Xbox bail chord (both triggers full + L3 + R3, one fresh press: `3 l3+r3 0 0 0 0 1 1`) wipes out on the spot; the skater gets back up where he fell after ~6 s. Riding into a bin or a wall at push speed only stops the skater.
- Repeatable starts: set a session marker right after the demo-path boot (`lb+down`) and return to it with `lb+up` held ~90 polls; bails and the demo path otherwise leave the skater wherever he stopped. `scenarios/grind.txt` then reaches the left handrail of the plaza stairs (~1 grind in 4 tries; the rest hit the rail or bail on the stairs, which is useful bail data too).
- Screenshots of the recomp: one PowerShell process looping `PrintWindow` (flag 3) every 100-200 ms, timed from the pad-script file's write time; starting a new process per shot takes ~3 s. Timing drifts by a few hundred ms per run (the marker return is not exact), so aim by looking, not by counting.
- `audio_trace_match.py` takes ~3 min on a long trace; cut the trace to the windows you need (a copy with only those lines) before matching.
- Menu sounds live check (SK-022): `Send-GameKeys` releases keys on game time, which the pause menu stops, so inside the menu only the first key press arrives and a held arrow key never releases (the cursor keeps repeating). Opening the menu and one move play `menu_open`/`menu_move` (Name `Sound menu_*` entities via `world.query`); closing is covered by the `menu_cue` unit test.
