# Skate 3 Rust Engine

Rust + Bevy reimplementation of Skate 3 (skating, tricks, grinds, offboard, `.skate` maps). Game assets are the user's own and never enter the repo. Machine-specific paths, build tips and the user's play-session rules live in `CLAUDE.local.md` (untracked).

## Agent skills

### Issue tracker

The ordna board in `tasks/` (cards `SK-nnn`). See `docs/agents/issue-tracker.md`.

### Workflow

Card state → next skill, announced at every state change. See `docs/agents/workflow.md`.

### Domain docs

Single-context: `CONTEXT.md` glossary + `docs/adr/`. See `docs/agents/domain.md`.

## Code rules

These are the **Standards axis** of `/code-review`.

- **README rule**: every behaviour change is documented in `README.md` under "Changes in this fork", in the same commit, one bullet per feature/fix naming its card. Update or remove the bullet when the behaviour changes.
- **Frameworks**: when a feature touches an area later cards will touch too (HUD/UI, mod host APIs, controller bindings, launchers, audio events), extract a small reusable layer (helpers, a module, a Lua SDK helper) so the next feature is a few lines on top of it. Extract once a second use is real or planned on the board; name the framework in the summary and the card. `/codebase-design` holds the vocabulary (module, interface, seam, depth).
- **Formatting**: the codebase does not follow rustfmt. Match the surrounding style and format only the lines you write.
- **Bevy `=0.18.1`** with patched `bevy_pbr` and `bevy_core_pipeline` in `vendor/`. Check every Bevy API against the vendored or registry source before relying on memory; 0.19 examples do not compile here.
- **Verification**: `cargo check -p <crate> --locked` while iterating; `cargo test --locked -p skate-mods -p skate-vehicles -p skate-net` before a commit; in-game behaviour through `scripts/GameTest.psm1` scenarios or BRP screenshots (`docs/` per topic).
- **Implementation notes**: one file per topic in `docs/` (e.g. `docs/grind-integration.md`); mod authoring starts at `sdk/AGENTS.md`.
- **Research**: findings for a card go in `.claude/card-research/<card>-<name>/README.md` (untracked); the card links to it. The Skate3Recomp source and the retail shaders are the reference for how the original game behaves.

## Git

- Work happens on `dev/brp`. `main` holds only work the user approved in game; approved cards are merged from `dev/brp` into `main`.
- Never commit game files: anything extracted or converted from the disc (`data/`, `assets/`, `maps/private/`, `*.skate`, `.xex`/`.iso`, textures, animations, audio, HUD exports, settings with install paths). Review `git status` before every commit.
- Commit messages name the card (`… (SK-nnn)`); a hook enforces this together with the README rule (`docs/agents/workflow.md`).
