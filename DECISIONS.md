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

## D16. Port the OpenFront engine instead of the original design
- **Date:** 2026-11-20
- **Choice:** laukofront ports the OpenFront engine (<https://github.com/openfrontio/openfrontio>)
  to Rust. The territory-graph sim (Stages 4–7) is replaced; the unfinished Stage 8 territory
  renderer was shelved in `git stash` ("Stage 8 territory renderer (superseded ...)").
- **Reason:** the owner's request. OpenFront is open source, so its real rules can be ported.
- **Kept:** repo, workflow, docs structure, config loading, benchmark harness, Bevy camera code.
- **Order:** core loop first (P1–P5 in `PLAN.md`); boats, structures, alliances, nations, nukes later.

## D17. Licence: AGPL-3.0-only (replaces D8, MIT)
- **Date:** 2026-11-20
- **Choice:** the whole project is AGPL-3.0-only with OpenFront's section 7 additional terms
  (`LICENSE`, copied unchanged from upstream). `NOTICE.md` records origin and attribution.
- **Reason:** a port of AGPL code is a derivative work and must use the same licence. The section 7
  terms require keeping "© OpenFront and Contributors" visible and not using "OpenFront" as the
  primary title.
- **Consequence:** anyone receiving the game (or playing it over a network) is entitled to the
  source. Releases must ship `LICENSE` and `NOTICE.md`.

## D18. Name: laukofront
- **Date:** 2026-11-20
- **Choice:** crate, binary and window title `laukofront` (was `openfront_clone`), chosen by the owner.
- **Reason:** the old name used "OpenFront" as the primary title, which the section 7 terms forbid.

## D19. Upstream reference clone
- **Date:** 2026-11-20
- **Choice:** `reference/openfrontio/` is a sparse, shallow clone of the upstream engine packages
  at commit `129d4b32a7dd49720ed8a70ebef7c70770948aaf`, git-ignored. Nothing in it is executed
  (no `npm install`); it is read only.
- **Reason:** porting needs the exact source; pinning one commit keeps the port consistent while
  upstream changes daily.

## D20. Own maps, art and UI; engine logic only
- **Date:** 2026-11-20
- **Choice:** only engine logic is ported. Maps come from laukofront's own terrain generator;
  colours, UI and any art are our own. No files from upstream `resources/` or `proprietary/`.
- **Reason:** upstream maps and art are separate assets (CC BY-SA, some all-rights-reserved), and
  laukofront should be its own game built on the engine rather than a copy of OpenFront's
  presentation.

## D21. Reference values for the ported RNG and DetMath
- **Date:** 2026-11-20
- **Choice:** the Rust tests compare against bit patterns produced by a hand transcription of
  upstream `PseudoRandom.ts` and `DetMath.ts` run in Node.js (not by executing the downloaded
  upstream files).
- **Reason:** proves the Rust port reproduces upstream's JavaScript integer and float semantics
  (`Math.imul`, `>>>`, `| 0`, IEEE doubles) bit for bit, so ported rules stay deterministic.

## D22. Terrain generator shape
- **Date:** 2026-11-20
- **Choice:** fractal value noise minus a rounded edge falloff, 50% land, magnitude curve
  `pow(h, 1.6)`. See `docs/MAP_GENERATION.md`.
- **Reason:** a first try with a square edge fade produced rectangular continents; the blended
  round/square falloff gives coastlines with bays and peninsulas.

## D23. Tile sets as vectors with a game-wide slot index
- **Date:** 2026-11-20
- **Choice:** each player keeps `tiles` and `border` vectors; `Game` keeps each tile's index in its
  owner's vectors, giving O(1) add/remove (upstream uses a hash-based `TileSet`).
- **Reason:** a tile has exactly one owner, so one index per tile is enough and needs no hashing.
- **Consequence:** iteration order differs from upstream. The rules stay the same, but a laukofront
  match is not tick-for-tick identical to the same match in OpenFront. Nothing needs that.

## D24. Old territory sim removed; benchmark and AI config replaced
- **Date:** 2026-11-20
- **Choice:** P2 deletes the Stage 6 modules (`core`, `map`, `territory`, `combat`, `economy`,
  `sim`, `ai`), their tests, `sim_bench` and `assets/config/ai.toml`. `game.toml` now holds the
  ported rules. `engine_bench` replaces `sim_bench`.
- **Reason:** they modelled the abandoned territory-graph design (D16). Git history keeps them.
  Upstream's AI settings will come back with the bot and nation milestones.

## D25. Encirclement rule simplified
- **Date:** 2026-11-20
- **Choice:** `engine::clusters` ports the main rules of upstream `PlayerExecution.removeClusters`
  (single-enemy test for the largest piece, surrounded test and captor choice for the others) on
  4-connected pieces of the whole territory. Upstream's diagonal-pinch handling and its
  border-tile clustering are not ported.
- **Reason:** upstream's version is ~1 200 lines; the main rules cover normal play.
- **Consequence:** rare diagonal-pinch cases can resolve differently. Flood-filling whole
  territories costs up to ~3 ms on a single tick on the largest map (see PERFORMANCE.md); porting
  upstream's border-tile clustering is the fix if Stage 15 needs it.

## D26. What the attack port leaves out
- **Date:** 2026-11-20
- **Choice:** attacks are ported without alliances (nobody is friendly), embargoes, relations,
  defense posts, fallout, boat attacks, and stats other than "attacks sent". Attack ids are
  numbers (indexes into the game's attack table) instead of upstream's random strings.
- **Reason:** those features belong to later milestones; the hooks (`DefenderInfo.is_traitor`,
  `defender_has_defense_post`, `fallout_ratio`, `source_tile`) are in place.

## D27. Bots without boats
- **Date:** 2026-11-20
- **Choice:** the tribe AI is ported for land only; boat attacks and shore-reachable neighbours
  return "no attack" until transport ships are ported.
- **Consequence:** on maps where survivors end up on different landmasses the match stalls and is
  decided by the 170-minute limit. Measured with `bot_match` (10 bots, Small): 6 of 8 seeds end by
  conquest after 32–64 game minutes; seeds 1 and 5 stall (D28 tracks the fix).

## D28. Transport ships are the next engine milestone after the client
- **Date:** 2026-11-20
- **Choice:** boats (upstream `TransportShipExecution` and water pathfinding) come right after P5.
- **Reason:** water separates landmasses on most generated maps; without boats matches stall.

## D29. Map drawn as one texture
- **Date:** 2026-11-20
- **Choice:** the map is a single RGBA texture (one texel per tile, nearest sampling) on a sprite.
  Each tick only the tiles that changed owner and their four neighbours are repainted in the CPU
  copy; Bevy uploads the texture only on ticks with changes.
- **Reason:** simplest correct renderer for tile maps up to 1.6 M tiles; upstream's WebGL renderer
  also works per tile. Measured at ~1 250 FPS on the dev PC (PERFORMANCE.md).
- **Rejected:** the Stage 8 mesh renderer (built for territories, not tiles); a custom shader
  (more code than needed for now).

## D30. Copyright notice text
- **Date:** 2026-11-20
- **Choice:** the in-game notice reads "Copyright (c) OpenFront and Contributors" instead of the
  © sign.
- **Reason:** Bevy's built-in font has no © glyph (it rendered as a box). "Copyright" is the legal
  equivalent. When a font with © is added (Stage 14, with its licence in CREDITS), the notice
  switches to the exact wording "© OpenFront and Contributors".

## D31. Automated runs do not take keyboard focus
- **Date:** 2026-11-20
- **Choice:** with `--perf-run` or `--screenshot`, the window opens unfocused and Bevy runs
  continuously while unfocused.
- **Reason:** the first screenshot showed the game paused and panned by keystrokes meant for
  another window; and unfocused windows are throttled to 60 FPS by default, which hid the real
  frame time.
