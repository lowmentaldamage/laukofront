# Changelog

Versions follow the build guide milestones. `1.0.0` is the finished game.

## 0.1.0 — 2026-11-20 (unreleased)

- Stages 0–3: repository, environment record, folder layout, Bevy 0.20.1 project.
- Stage 4: `PLAN.md` and config files with every balance number.
- Stage 5: design documents, decisions log, MIT license.
- Stage 6: headless simulation core (map generation, territories, combat, economy, tick) with tests.
- Stage 7: headless benchmark `sim_bench` and first performance results.

## 0.2.0 — 2026-11-20 (unreleased)

- Renamed to **laukofront**; relicensed to AGPL-3.0-only (OpenFront port, D16–D20).
- New `PLAN.md` for the OpenFront engine port; `NOTICE.md` with attribution.
- P1: ported PRNG, DetMath, tile map; own terrain generator; `map_preview`.
- P2: players, spawn phase and spawning, troop growth and gold (upstream formulas); `engine_bench`;
  old territory sim removed.
