<p align="center">
  <img src="docs/images/skating-crab.png" alt="Rust crab riding a skateboard" width="480">
</p>

# Skate 3 Rust Engine

A Rust and Bevy skating project built from Skate 3 reverse-engineering research.
Includes skating, tricks, grinds, offboard movement, difficulty settings and
`.skate` map support. Gameplay parity is still a work in progress.

## Mods and agent authoring

Drop mod ZIPs into top-level `mods/` and enable them in the Mods menu. **Point your coding agent at [sdk/AGENTS.md](sdk/AGENTS.md)**
to make a mod. See [package structure](docs/mod-packages.md), [Lua API](docs/lua-modding.md),
[vehicle API](docs/vehicle-sdk.md), [multiplayer mod SDK](docs/multiplayer-mods.md) and [Mixamo workflow](docs/mixamo-vehicle-workflow.md).
Editable examples live in `sdk/examples/`; `tools/package_mod.py` validates and packages them.

## Play

[Download Experimental](https://github.com/SK8-ENGINE/skate-3-rust-engine/releases/tag/experimental).
Successful `main` builds replace this prerelease. Choose **Latest** in Updates
for experimental updates; **Stable** is the default.

Extract the Windows release ZIP and run `skate3rust.exe`. Select your Skate 3
Xbox 360 ISO, or select `default.xex` in an extracted game folder. Keep its
`data` folder alongside it. Setup prepares the skater, animations and all disc maps, then
starts University. The original scoring and session-marker HUD assets are also
exported automatically during setup. No Blender, Python or Rust installation is needed.
ISO extraction needs internet access. The first conversion can take a while.

Use an XInput controller to play. Escape opens graphics, difficulty and map
settings. Maps can be switched without restarting the game.

**Skate 3 assets are not included.** Your converted files stay in
the `data` folder beside your executable. Each freshly unpacked copy runs its
own setup; it does not adopt another installation. In-place updates refresh
only changed asset groups.

## Build

Requires Windows, Rust with the MSVC toolchain, and LLVM installed in its default
location. Run `BUILD.bat` to build, then `PLAY.bat` to launch the test world.
`PLAY.bat` opens your saved map (University by default); use the in-game menu to switch maps, or drag a `.skate` file onto `PLAY.bat`. An XInput controller is required for gameplay;
Escape opens difficulty and graphics settings.

Development builds use a prepared asset set in `assets/private/` or the
installed asset directory. `scripts/Build-Release.ps1` builds the portable Windows
package and requires Python 3.13. GitHub Actions builds `main` automatically;
numbered releases are published separately.

Custom animations and climbing support remain available, but no custom clips
are shipped. The included format-demo map is original procedural content.

Implementation notes are in [`docs/`](docs/). Patched Bevy dependencies and
their licenses are in [`vendor/`](vendor/). This is an unofficial project,
not affiliated with EA.

## Changes in this fork

Modifications on top of upstream (branch `dev/brp`). The feature board lives in
[`tasks/`](tasks/) (`ordna`); each entry names its card.

**Controls and menus**
- **F2 acts as Escape**: it opens the pause menu and backs out of menus.
- **Game options** page in the pause menu (Right-stick indicator, Fly mode), saved to `settings/game-options.json`.
- **Fly / noclip mode** (SK-026): press **B** (or F3) while on foot. Use the left stick to move, LB/RB to go down/up, and B again to land. Physics is paused while flying.
- **NPC pedestrians** (SK-034, first version): a few characters from the retail roster (or the stock skater) wander around the player on foot, playing the original walk and stand cycles with the walk speed taken from the clip so feet do not slide. They stay on walkable ground and stop at walls and drops (line probes against the collision world, `world_probe.rs`); no physics or interaction yet. Count in Game options ("Pedestrians", 0-12, default 4) or `SKATE3_PEDESTRIANS=<n>`; they respawn around the player after a map change or teleport. The rendering is the new **puppet** layer (`puppet.rs`), shared with multiplayer remote skaters: spawn a look, bind its joints, write a pose each frame. No measurable FPS cost with 10 on the test world (87.8 vs 87.4 FPS). Live check: `scripts/Test-Pedestrians.ps1`.

- **Two players side by side** (SK-002): `scripts/Launch-TwoPlayers.ps1` starts a host and a guest over local direct multiplayer, each borderless on one half of the primary monitor and tied to its own controller (BRP ports 15703/15704, logs in `logs/2p/`). Options: `-Windowed`, `-Release`, `-MapPath`.
- New launch flags `--window X Y WIDTH HEIGHT` and `--borderless`.
- In multiplayer, menus read only the window's own controller (`--controller`), or only the focused window without it.

- **Sound framework** (SK-021): `crates/skate-game/src/audio/` plays the decoded original sounds. `events.json` maps event names to sounds (one line per sound); one-shots via `PlaySound`, loops via `SoundLoops::set` each frame. Master/Music/Effects volumes in Game options. `SKATE_AUDIO_TEST=<event>` plays an event at startup. Gameplay sounds (SK-022): pop, landing, grind start and bail are triggered from physical state changes, plus a rolling loop that follows board speed, a wind loop above 4 m/s and a grind loop (SK-023). An event's `layers` play with it, so pop and landing stack tail, trucks, board and air samples like the original. Pop, landing, rolling, wind and the pause menu sounds (open, close, move; any menu can feed `audio::menu::menu_cue`) use the samples the original game plays, found with the SK-032 trace; grind and bail picks are still provisional. `python tools/audio_browser.py assets` writes a local page (in your data folder) to listen to every decoded sound by id.

**HUD**
- **Right-stick indicator** at the bottom centre. It uses the original Trick Analyzer ring and draws a line along each flick that fades after the trick. When the original recognizer accepts a right-stick trick, the pattern's ideal path is drawn in amber with the trick name above the ring (SK-027).

**Mods and SDK**
- **D-pad Tools** example mod (`sdk/examples/dpad-tools`):
  - D-pad left saves a spot; D-pad right returns to it.
  - D-pad up/down changes push speed on the board and running speed on foot.
  - Its window starts collapsed and its HUD starts hidden.
- **Speedometer** example mod (`sdk/examples/speedometer`, SK-007): speed on the board and on foot plus top speed since the map loaded; km/h, mph or m/s; can be hidden in its settings.
- `sdk.trainer.apply` has a new `run_speed` field (0.25..4) that scales on-foot walk, run and sprint speed (SK-025).
- `sdk.physics.gravity(scale)` changes world gravity for mods (0.25..2), with its own owner; resets when the mod stops (SK-004).
- `sdk.player.scale(s)` changes how big the skater and board look (0.5..2, visual only: collision, speed and jumps stay stock), with its own owner; example mod **Skater Size** (`sdk/examples/skater-size`) sets it from a slider (SK-038).
- `sdk.player.impulse({x,y,z})` gives the skater an instant velocity boost while riding or in board air (SK-005).
- Mod manifests can set `start_collapsed` so the mod's settings window opens minimized.
- Fixed the native action IDs listed in [docs/lua-modding.md](docs/lua-modding.md).

**Graphics**
- 16x anisotropic filtering on world textures.
- Render scale up to 200% (supersampling), since MSAA stops at 8x.
- Performance (SK-018): `scripts/Bench-Graphics.ps1` A/B benchmark with `SKATE_RENDER_SCALE`/`SKATE_MSAA`/`SKATE_FPS_LIMIT`/`SKATE_CHARACTER_SHADOW` overrides; the character shadow map uses 2 cascades to 40 m instead of 4 to 100 m. Results in [docs/performance.md](docs/performance.md).
- Controller polling no longer queries empty XInput slots every frame (re-checked once per second): main schedule ~3 ms faster, about +20% FPS on University (SK-028). `SKATE_PERF_REPORT` now records the GPU adapter and effective graphics settings; `Bench-Graphics.ps1 -Repeats N` prints mean and deviation.

**Assets and tooling**
- Setup has a new optional `audio` group (SK-020): it downloads vgmstream-cli (pinned SHA) and decodes 16 original sound banks (grinds, wheels, scrapes, footsteps, board collisions, metal hits, foley, menu...), including EA "SPLC" `.bnk` banks read by `tools/asset_pipeline/splc.py` (reverse engineered: each sample is an EA SNR vgmstream decodes), to `assets/private/audio/` with an `audio.json` index. A failure only disables sound.
- **MW2 characters as skaters** (SK-036): a separate local tool (`iw4-character-export`, not in this repo) turns Modern Warfare 2 soldiers from your own install into Mixamo-named GLBs, which Custom models imports as-is (Soap and a TF141 soldier tested). Nothing extracted from MW2 is committed. Custom models can also be imported without the picker: `python tools/setup.py --character-import --library-import <library> --reference assets/private/skater.glb --result <reply.json> --noninteractive <model.glb>`.
- Setup also exports the original front-end UI textures (`hud2/…`) for HUD use.
- **DLC maps** (SK-009): `scripts/Convert-DlcMaps.ps1` converts your own official DLC districts (Maloof Money Cup DLC, Zen, AG Park, Sanitarium, Back Lot, Downtown Skatepark Night, DW Mega Compound) from the packages Skate3Recomp extracted; they appear in the map menu. `convert_map`/`map_job.py` take `--district`/`--label` for multi-district packages.
- **Skate3Recomp XMA trace** (SK-032): a patched ReXGlue runtime logs every XMA buffer the original game decodes; `tools/audio_trace_match.py` maps them to our sound ids, so each event's real sample is found by playing the original. It can run without a player: `scripts/Run-Skate3RecompTrace.ps1 -Automate` boots straight into gameplay and the runtime drives the pad from a script file with markers (`tools/recomp_trace/scenarios/`), and `--recurring <action> <seconds>` lists the ids that follow every repetition of an action. See [tools/recomp_trace](tools/recomp_trace/README.md).
- Bevy Remote Protocol behind the `brp` feature, for live inspection.
- Scripted play test harness (`crates/skate-game/src/tests/scripted_play.rs`) that drives the physics from a raw controller script; notes in [docs/testing-notes.md](docs/testing-notes.md).

**Fixes**
- **Crash recovery** (SK-033): a physics failure no longer closes the game. The report is saved to `logs/crash-<time>.txt`, the current map is reloaded and the skater is put back where it was about 2 s earlier; the pause menu status shows the report path. `SKATE_FAIL_FAST=1` (and `--verify`) keep the old exit-with-error behaviour; `SKATE_FORCE_PHYSICS_FAILURE=<seconds>` fakes one failure for testing.
- Board landing crash (SK-031): on x86, a subnormal squared length (vector lanes around 1e-20 after a landing) made `length()` infinite and the ground up vector NaN, which the physics rejected and closed the game. The Xbox 360 VMX unit flushes denormals to zero; `length()` and the ground-orientation normalize now do the same.
- On-foot crash after landing (SK-025): a leftover 4th (w) component in the skater's position fed back through the moving-support velocity and doubled every tick until the physics failed. Faster running made it more likely. The support velocity now has no w component.

## Advanced diagnostics

Windows builds support opt-in [performance timeline capture](docs/performance-tracing.md)
through the `--trace` CLI option, including optional GPU pass diagnostics.
