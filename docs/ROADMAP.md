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
| P1 Engine lib | sfc32 matches upstream; DetMath tested; generated maps deterministic |
| P2 Players, economy, spawn | growth/cap/gold match upstream formulas; spawn claims tiles |
| P3 Attacks and win | tile-by-tile conquest; retreat; 80% win; invariants over long runs |
| P4 Bots | a bots-only match reaches a winner |
| P5 Bevy client | texture map, click to spawn/attack, HUD |

## Future

| Milestone | Accepted when |
|---|---|
| Upstream features | boats, structures, alliances, nations, nukes, warships, trade — one milestone each |
| Build-guide Stages 12–20 | menus, save/load, polish, performance, QA, package, final docs, tag `v1.0.0` |
