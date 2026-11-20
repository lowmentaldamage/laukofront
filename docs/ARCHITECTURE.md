# Architecture

## Modules and what they own

| Module | Owns | Bevy? |
|---|---|---|
| `core` | `TerritoryId`, `PlayerId`, `Owner`, `GameError`, `OrderError`, `Rng` (SplitMix64), constants | no |
| `config` | `GameConfig`, `AiConfig`, `MapSize`; loading and validating the TOML files | no |
| `map` | `Map`: cell raster, seed points, areas, centroids, adjacency (CSR) | no |
| `territory` | `Territory` and `PlayerState` plain data | no |
| `combat` | `resolve()`: one attack against one territory; `AttackOutcome` | no |
| `economy` | troop caps, growth, gold income | no |
| `sim` | `World` (all match state), `Order`, `Event`, `VictoryReason`, the tick | no |
| `ai` | AI players; today `RandomAttacker` (stand-in), Stage 11 the real AI | no |
| `render` | map mesh, colours, borders, camera (Stage 8) | yes |
| `input` | mouse/keyboard → selection and orders (Stage 9) | yes |
| `ui` | HUD panels and notifications (Stage 10) | yes |
| `menu` | menus, match setup, end screen (Stage 12) | yes |
| `main.rs` | builds the Bevy `App` | yes |
| `bin/sim_bench.rs` | headless benchmark | no |

## Allowed dependencies

```
core
 └─ config
     └─ map
         └─ territory
             ├─ combat
             └─ economy
                 └─ sim  (uses combat + economy)
                     └─ ai
                         └─ render, input, ui, menu  (Bevy)
                             └─ main.rs
```

A module may only use modules above it in this tree. In particular:
- Nothing below `sim` is allowed to know about Bevy. The sim must build and test without a window.
- `config` does not depend on `economy` or `sim`.
- `render`, `ui` and `menu` only **read** `World`. The only ways to change a match are
  `World::queue_order` (normal play), `World::tick`, and `World::set_territory` / `set_gold`
  (setup, tests, save loading).
- `ai` produces `Order`s through `World::queue_order`, like the human.

## Simulation tick vs frame

- `World::tick()` advances exactly one 100 ms step. It does not know about real time.
- From Stage 8, Bevy's `FixedUpdate` schedule calls `tick()` at 10 Hz × game speed.
- Rendering runs every frame. It reads `World::events()` after each tick to find territories whose
  owner changed and rewrites only their vertex colours.

## Data flow

```
mouse/keys ─► input ─┐
                     ├─► World::queue_order ─► (next tick) World::tick ─► events ─► render / ui / audio
AI (every 1 s) ──────┘
```

## Tick order (`sim::World::tick`)

1. Stop if the match is over.
2. Clear last tick's events.
3. Resolve queued orders in queue order (re-checking ownership and troops).
4. Tick counter + 1.
5. Recompute troop caps; on every 10th tick, grow troops.
6. Refresh player totals; pay gold.
7. Eliminate players with no territory; check conquest and time-limit victory.

## Rules for simulation code

- No Bevy types in `core`, `config`, `map`, `territory`, `combat`, `economy`, `sim`.
- No `unwrap()` / `expect()` outside tests. Errors are `GameError` / `OrderError`.
- No allocation inside the tick once buffers are warm: orders are double-buffered
  (`orders` / `resolving` swap), events reuse one `Vec`, caps and totals are recomputed in place.
- Integer maths only in the sim (troops `u32`, gold in milli-gold `u64`, distances in `i64`),
  so results are identical on every machine.
