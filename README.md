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

**HUD**
- **Right-stick indicator** at the bottom centre. It uses the original Trick Analyzer ring and draws a line along each flick that fades after the trick.

**Mods and SDK**
- **D-pad Tools** example mod (`sdk/examples/dpad-tools`):
  - D-pad left saves a spot; D-pad right returns to it.
  - D-pad up/down changes push speed on the board and running speed on foot.
  - Its window starts collapsed and its HUD starts hidden.
- `sdk.trainer.apply` has a new `run_speed` field (0.25..4) that scales on-foot walk, run and sprint speed (SK-025).
- Mod manifests can set `start_collapsed` so the mod's settings window opens minimized.
- Fixed the native action IDs listed in [docs/lua-modding.md](docs/lua-modding.md).

**Graphics**
- 16x anisotropic filtering on world textures.
- Render scale up to 200% (supersampling), since MSAA stops at 8x.

**Assets and tooling**
- Setup also exports the original front-end UI textures (`hud2/…`) for HUD use.
- Bevy Remote Protocol behind the `brp` feature, for live inspection.

**Fixes**
- On-foot crash after landing (SK-025): a leftover 4th (w) component in the skater's position fed back through the moving-support velocity and doubled every tick until the physics failed. Faster running made it more likely. The support velocity now has no w component.

## Advanced diagnostics

Windows builds support opt-in [performance timeline capture](docs/performance-tracing.md)
through the `--trace` CLI option, including optional GPU pass diagnostics.
