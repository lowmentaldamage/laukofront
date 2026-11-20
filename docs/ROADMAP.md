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
| P7 Boats | transport ships and water routing; no more stalled matches (D28) |
| P8 Buildings | city, port, factory, defense post, trade ships, trains, build menu |
| P9 Warships and nukes | warships, missile silo, atom/hydrogen bomb, MIRV, SAM, fallout |
| P10 Diplomacy and nations | alliances, embargo, donations, nation AI with difficulty |
| P11 Maps | richer generator (biomes, rivers, styles) and real-Earth maps from Natural Earth |
| P12 Menus and modes | main menu, match setup, teams, Doomsday Clock, settings |
| P13 Save, audio, polish | snapshots, sound, end-of-match statistics |
| P14 Performance and release | border-tile encirclement, mid-range PC, zip, v1.0.0 |

## Future

| Milestone | Accepted when |
|---|---|
| Upstream features | boats, structures, alliances, nations, nukes, warships, trade — one milestone each |
| Build-guide Stages 12–20 | menus, save/load, polish, performance, QA, package, final docs, tag `v1.0.0` |
