# Decisions

One entry per decision: date, choice, reason, rejected options.

## D1. Project location
- **Date:** 2026-11-20
- **Choice:** `D:\Game-rust-openfront` instead of the guide's `D:\school-game_openfront-rust-project`.
- **Reason:** the work session was already set up in this folder; the owner chose to keep it.
- **Rejected:** creating the guide's path (would leave two folders).

## D2. Engine: Bevy 0.20.1 (exact pin)
- **Date:** 2026-11-20
- **Choice:** `bevy = "=0.20.1"`, the latest stable release on crates.io on this date.
- **Reason:** the guide fixes Rust + Bevy; an exact pin keeps builds reproducible because Bevy breaks its API between minor versions.
- **Rejected:** a caret range (`0.20`) — silent upgrades; older Bevy versions — no reason to start behind.

## D3. Helper crates
- **Date:** 2026-11-20
- `serde = "=1.0.229"` (derive): turns `game.toml` / `ai.toml` into typed structs, and later save files.
- `toml = "=1.2.1"`: parses the TOML config files.
- **No RNG crate:** a ~30-line SplitMix64 lives in `src/core/rng.rs`. Its output is fixed by our code, so the same seed gives the same map on every platform and every dependency upgrade.
- **Rejected:** `rand` + `rand_chacha` (larger, and their value stream may change across major versions); `thiserror` (one hand-written error enum is enough); `criterion` (a `std::time` bench binary is enough for Stage 7).

## D4. Toolchain
- **Date:** 2026-11-20
- **Choice:** `rust-toolchain.toml` pins `1.99.1`, the toolchain used in Stage 1. Edition 2021, as the guide asks.

## D5. Build profiles
- **Date:** 2026-11-20
- **Choice:** dev builds compile dependencies with `opt-level = 3` and our crate with `opt-level = 1`; release uses `lto = "thin"`, `codegen-units = 1`.
- **Reason:** unoptimized Bevy is too slow to play; release is the shipped build so it gets the extra optimization.

## D6. Smart App Control on the dev machine
- **Date:** 2026-11-20
- **Choice:** the owner turned Windows Smart App Control off.
- **Reason:** it blocked unsigned build scripts and proc-macro DLLs that cargo compiles (os error 4551), so `cargo build` could not finish.
- **Consequence:** players whose PCs have Smart App Control on may have the unsigned release `.exe` blocked. Noted for `RELEASE.md` and `PROJECT_STATUS.md`.

## D7. Empty Stage 3 app
- **Date:** 2026-11-20
- **Choice:** `main.rs` runs `App::new().run()`, which runs once and exits without opening a window.
- **Reason:** the guide's Stage 3 asks for an app that compiles and "opens nothing yet". The window arrives in Stage 8.

## D8. License: MIT
- **Date:** 2026-11-20
- **Choice:** MIT (`LICENSE.md`).
- **Reason:** short, permissive and widely understood; fits a school project; matches the MIT
  option of Bevy, serde and toml, so there is no license conflict.
- **Rejected:** Apache-2.0 alone (longer, patent clauses not needed); GPL (would force the same
  license on anyone reusing the code); no license (others could not legally reuse it).

## D9. Gold has a use: Reinforce
- **Date:** 2026-11-20
- **Choice:** added a Reinforce order: 200 gold → +25 troops on an own territory, up to its cap.
- **Reason:** the guide shows gold and income in the HUD but gives gold nothing to buy. Without a
  use, gold is a meaningless number.
- **Rejected:** buildings (too much scope before Stage 11); gold as score only.

## D10. Time-limit victory at 60 minutes
- **Date:** 2026-11-20
- **Choice:** `sim.max_match_ticks = 36000`; winner = most territories, then most troops, then
  lowest player id.
- **Reason:** guarantees every match (and the Stage 11 "finishes within N ticks" test) ends.

## D11. Easy AI handicap is slower thinking, not slower growth
- **Date:** 2026-11-20
- **Choice:** Easy thinks every 2 s and needs a 2.0× advantage; growth is the same for everyone.
- **Reason:** the guide suggests "slower growth" for Easy, but also says AI uses only the actions
  a human can use. Giving AIs different growth rules would make the sim treat players
  differently. Handicaps live in the AI's decisions instead.

## D12. Combat is a pure function on one territory
- **Date:** 2026-11-20
- **Choice:** `combat::resolve(attacker, sent, &mut Territory) -> AttackOutcome`. `World` checks
  the order and removes sent troops; `combat` only decides the fight.
- **Reason:** keeps `combat` below `sim` in the dependency tree and trivially testable.

## D13. Raster Voronoi map
- **Date:** 2026-11-20
- **Choice:** territories are regions of a cell raster (nearest seed point), adjacency = shared
  cell edge. See `docs/MAP_GENERATION.md`.
- **Reason:** adjacency is exact and symmetric by construction; the same raster gives O(1) mouse
  picking and the mesh for Stage 8; integer maths keeps it deterministic.
- **Rejected:** pure graph with no geometry (nothing to draw); continuous Voronoi with a geometry
  crate (float edge cases, extra dependency, harder to keep deterministic).

## D14. Stand-in AI for tests and the benchmark
- **Date:** 2026-11-20
- **Choice:** `ai::RandomAttacker`, a seeded random attacker that only uses `queue_order`.
- **Reason:** Stage 7 measures 1/3/5 AIs before the real AI exists (Stage 11). It is labelled as
  a stand-in everywhere; the Stage 11 AI replaces it in the benchmark.

## D15. Benchmark without criterion
- **Date:** 2026-11-20
- **Choice:** `src/bin/sim_bench.rs` using `std::time::Instant`, printing Markdown tables.
- **Reason:** no extra dependency; output pastes straight into `PERFORMANCE.md`.
