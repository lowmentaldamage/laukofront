# Roadmap

Milestones in order. Stage numbers refer to the build guide.

## Done

| Milestone | Stages | Result |
|---|---|---|
| M0 Skeleton | 0–3 | git repo, environment recorded, folder layout, Bevy project builds |
| M1 Plan and docs | 4–5 | PLAN.md (18 sections), all design docs |
| M2 Simulation core | 6 | plain-Rust sim, 45 tests pass |
| M3 Headless benchmark | 7 | 1.7 M ticks/s for 80 territories + 3 AIs (PERFORMANCE.md) |

## Next

| Milestone | Stages | Accepted when |
|---|---|---|
| M4 Playable map | 8–10 | map renders as one mesh; click to attack; HUD numbers match the sim |
| M5 AI | 11 | Easy/Normal/Hard AI; an AI-only match reaches a winner |
| M6 Match flow | 12 | menu → match → end screen → new match without restarting |

## Future

| Milestone | Stages | Accepted when |
|---|---|---|
| M7 Save, polish, performance | 13–15 | save/load (optional); audio and effects; budgets met on a mid-range PC |
| M8 Release | 16–19 | QA checklist dated; zip runs on a clean folder; tag `v1.0.0` |
| Later | — | multiplayer (see NETWORKING.md), more map shapes, water/terrain |
