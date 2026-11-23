# Roadmap

Milestones in order. Stage numbers refer to the build guide.

## Done

| Milestone | Stages | Result |
|---|---|---|
| M0 Skeleton | 0–3 | git repo, environment recorded, folder layout, Bevy project builds |
| M1 Plan and docs | 4–5 | PLAN.md (18 sections), all design docs |
| M2 Simulation core | 6 | plain-Rust sim, 45 tests pass |
| M3 Headless benchmark | 7 | 1.7 M ticks/s for 80 territories + 3 AIs (PERFORMANCE.md) |

## Port of the OpenFront engine (D16)

| Milestone | Accepted when |
|---|---|
| P0 Rename, relicense, plan | **done** — builds as `laukofront`; LICENSE/NOTICE in place |
| P1 Engine lib | **done** — sfc32 and DetMath match upstream bit for bit; deterministic map generator |
| P2 Players, economy, spawn | **done** — growth/cap/gold match upstream formulas; spawn phase and spawning |
| P3 Attacks and win | **done** — tile-by-tile conquest (upstream attackLogic), retreat, takeover, encirclement, 80% win |
| P4 Bots | **done** — tribe AI; bots-only matches reach a winner (by conquest when the map allows, D27) |
| P5 Bevy client | **done** — texture map, click to spawn/attack, keys, HUD, leaderboard |
| P6 Interface | **done** — left HUD, minimap, on-map tutorial, own-land highlight, labels, capture flash, end screen |
| P7 Boats | **done** — transport ships, water routing, boat retreat, bots use boats |
| P8 Buildings | **done** — city, port, factory, defense post, upgrades, trade ships, factory income, build bar |
| P9 Warships and nukes | **done** — silo, SAM, warships, atom/hydrogen bomb, MIRV, fallout |
| P10 Diplomacy and nations | **done** — alliances, traitors, embargo, donations, targets, reactions, nation AI with 4 difficulties |
| P11 Maps | **done** — styles, rivers, peaks, biomes; World, Europe, Asia, Africa, Americas, British Isles |
| P12 Menus and modes | **done** — main menu, setup, pause, settings, teams (no Doomsday Clock, no presets, D44) |
| P13 Save, audio, polish | **done** — replay saves (D43), generated sound effects (D44), end-screen stats |
| P14 Performance and release | **done** — v1.0.0: QA checklist, Windows zip with source, tag |

## Future

| Milestone | Accepted when |
|---|---|
| Upstream features | boats, structures, alliances, nations, nukes, warships, trade — one milestone each |
| Build-guide Stages 12–20 | menus, save/load, polish, performance, QA, package, final docs, tag `v1.0.0` |
