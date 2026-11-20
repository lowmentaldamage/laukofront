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
- P3: attacks (upstream attackLogic, tile queue), retreat, opposing/merged attacks, spawn immunity,
  defeated-player takeover with gold, encirclement (simplified), 80% win check.
- P4: bot AI (upstream tribe behaviour, land only); `bot_match` tool.
- P5: Bevy client — map texture, camera, click to spawn/attack, attack size, retreat, pause, speed,
  HUD, leaderboard, attribution; `--autoplay`, `--perf-run`, `--screenshot`.
- P6: interface overhaul — left HUD cards, clickable attack size and speed, retreat buttons,
  minimap, on-map tutorial, gold own-land border and YOU marker, map labels, capture flash, end
  screen with restart, notifications, saved settings.
- P7: transport ships with water routing, boat retreat, bots use boats.
- P8: cities, ports, factories, defense posts with upstream costs; upgrades; trade ships; factory
  income; build bar; ship and building icons; measured gold income in the HUD.
- P9: missile silos, SAM launchers, warships, atom and hydrogen bombs, MIRVs, fallout; target
  preview, explosions, SAM and shell effects, nuke warnings.
- P10: alliances (request, accept, renew, break), traitors, relations, embargoes, donations,
  targets, reactions; AI nations with four difficulties; diplomacy menu and panel; ally and
  traitor colours on the map and leaderboard.
