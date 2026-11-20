# Project status

**Date:** 2026-11-20
**State:** playable against bots with a full interface (port milestones P0–P6). Next: boats (P7), then buildings, warships and nukes, diplomacy and nations, maps, menus, save/audio, release (P8–P14; see ROADMAP).
The game is **not playable yet**: there is no window, map rendering or input. The simulation
rules work and are tested headless.

## Done

| Stage | Result |
|---|---|
| 0 Folder | git repo in `D:\Game-rust-openfront` |
| 1 Machine | `docs/ENVIRONMENT.md` |
| 2 Layout | full folder structure |
| 3 Rust project | Bevy 0.20.1 project builds; `main.rs` is an empty app that exits |
| 4 Plan | `PLAN.md`, all 18 sections; numbers in `assets/config/*.toml` |
| 5 Docs | every design doc in `docs/`, `DECISIONS.md`, `LICENSE.md` |
| 6 Sim core | `core`, `config`, `map`, `territory`, `combat`, `economy`, `sim`; tests pass |
| 7 Benchmark | `sim_bench`; results in `docs/PERFORMANCE.md` |

## Not done

- Stages 8–20: rendering, interaction, HUD, real AI, menus, save/load, audio, performance pass,
  QA, packaging, final docs, release tag.
- `ai.toml` is loaded and validated, but no AI uses it yet (Stage 11).
- `render`, `input`, `ui`, `menu` modules are empty placeholders.

## Known issues

- No menu yet: match settings come from the command line (CONTROLS.md).
- The in-game notice uses "Copyright (c)" until a font with © is added (D30).

- Encirclement is a simplified port (D25); worst single tick ~3 ms on the largest map.
- No boats yet: players on separate landmasses cannot reach each other (D27, D28).

- The dev machine is high-end (Ryzen 9 7950X, RTX 4090); the "mid-range PC" frame target has not
  been measured on mid-range hardware.
- Smart App Control had to be turned off on the dev machine to build. The release exe is
  unsigned, so players with Smart App Control on may have it blocked (see `docs/RELEASE.md`).
- Benchmark AIs are a random stand-in, not the Stage 11 AI.
- The full list of transitive crate licenses is not generated yet (planned for Stage 17).
