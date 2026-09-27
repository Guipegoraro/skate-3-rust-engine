# Skate 3 Rust Engine

Rust + Bevy reimplementation of Skate 3 (skating, tricks, grinds, offboard, `.skate` maps). Game assets are the user's own and never enter the repo. Machine-specific paths, build tips and the user's play-session rules live in `CLAUDE.local.md` (untracked).

## Agent skills

### Issue tracker

The ordna board in `tasks/` (cards `SK-nnn`), git-ignored so it is the same on every branch. See `docs/agents/issue-tracker.md`.

### Workflow

Card state → next skill, announced at every state change. See `docs/agents/workflow.md`.

### Domain docs

Single-context: `CONTEXT.md` glossary + `docs/adr/`. See `docs/agents/domain.md`.

## Code rules

These are the **Standards axis** of `/code-review`.

- **README rule**: every behaviour change is documented in `README.md` under "Changes in this fork", in the same commit, one bullet per feature/fix naming its card. Update or remove the bullet when the behaviour changes.
- **Reuse before rewrite**: before writing a helper (math, deadzone, teleport, state test, RNG), search `crates/skate-core` and `crates/skate-game/src/{input,replay.rs,modding}` for it and link the reused symbol in the card. A second copy of an existing function is a review finding.
- **Name the predicate**: gameplay code asks `PhysicalStateId::category()`/`is_grind()` or a `SkaterRuntime` method; raw native fields (`state_16`, `flag_69`, `category_12`) and numeric state literals stay inside `physics/` and `skate-core`.
- **Constants have one home**: bone indices, render layers, tick rate, native ids live in a documented `pub const`; the tick dt comes from `physics.clock.period()`; debug env vars are read once at startup into a resource. Any string identifier referenced from Rust (sound event, clip name) is asserted by a test to exist in its table.
- **One system, one concern**: a system stays under 60 lines and 12 parameters; a tuple added only to fit the parameter limit means split the system. Every-frame reconcilers declare a `run_if` (resource changed, message received) or write with `set_if_neq`. Plugins register in `build()` and read resources in `Startup` or `FromWorld`.
- **Pure core, thin shell**: each new module keeps its logic in a `struct`/`fn` tested without assets (as `Wander`, `Probe`, `FootPlants` do); the ECS system is the shell around it.
- **Player-facing text is English.**
- **Frameworks**: when a feature touches an area later cards will touch too (HUD/UI, mod host APIs, controller bindings, launchers, audio events), extract a small reusable layer (helpers, a module, a Lua SDK helper) so the next feature is a few lines on top of it. Extract once a second use is real or planned on the board; name the framework in the summary and the card. `/codebase-design` holds the vocabulary (module, interface, seam, depth).
- **Formatting**: the codebase does not follow rustfmt. Match the surrounding style and format only the lines you write.
- **Bevy `=0.18.1`** with patched `bevy_pbr` and `bevy_core_pipeline` in `vendor/`. Check every Bevy API against the vendored or registry source before relying on memory; 0.19 examples do not compile here.
- **Verification**: `cargo check -p <crate> --locked` while iterating; `cargo test --locked -p skate-game -p skate-mods -p skate-vehicles -p skate-net` and `node scripts/clippy-touched.js` (clippy on the lines you touched only, since upstream code carries its own warnings) before a commit; in-game behaviour through `scripts/GameTest.psm1` scenarios or BRP screenshots (`docs/` per topic).
- **Implementation notes**: one file per topic in `docs/` (e.g. `docs/grind-integration.md`); mod authoring starts at `sdk/AGENTS.md`.
- **Research**: findings for a card go in `.claude/card-research/<card>-<name>/README.md` (untracked); the card links to it. The Skate3Recomp source and the retail shaders are the reference for how the original game behaves.

## Git

- Work happens on `dev/brp`. `main` holds only work the user approved in game; approved cards are merged from `dev/brp` into `main`.
- Never commit game files: anything extracted or converted from the disc (`data/`, `assets/`, `maps/private/`, `*.skate`, `.xex`/`.iso`, textures, animations, audio, HUD exports, settings with install paths). Review `git status` before every commit.
- Commit messages name the card (`… (SK-nnn)`) when the work belongs to one.
