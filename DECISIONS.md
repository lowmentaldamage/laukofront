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

## D32. P6 interface layout
- **Date:** 2026-11-20
- **Choice:** one left column of cards (troops, gold, land, attacks, leaderboard, notifications);
  clock and speed top right; minimap bottom right (not bottom left as first planned, because the
  left column is tall); hover info bottom centre; key help only on demand (H / ?), replaced at the
  start by the on-map tutorial.
- **Reason:** the owner's play-test asked for the HUD on the left, better organised numbers, and a
  tutorial instead of an always-visible help line.

## D33. Simpler label placement than upstream
- **Date:** 2026-11-20
- **Choice:** a player's label sits at the average position of its tiles (moved onto its own land
  if that falls outside), sized by `sqrt(tiles)`, recomputed every 10 ticks. Upstream's
  `NameBoxCalculator` (largest inscribed box) is not ported.
- **Reason:** cheap (one pass over owned tiles) and good enough for compact territories; long
  curved territories may get a label off-centre.

## D34. Settings file location
- **Date:** 2026-11-20
- **Choice:** `%APPDATA%\laukofront\settings.toml` (or `$HOME/.config/laukofront/`), TOML,
  defaults when missing or broken. Holds `tutorial_done` and `default_attack_ratio` for now.

## D35. Water routing
- **Date:** 2026-11-20
- **Choice:** water bodies are labelled once (4-connected). Routes: A* on a coarse grid of 8x8-tile
  cells, then short weighted-A* legs between water tiles near every 4th coarse cell; full weighted
  A* only for short routes or if a leg fails. Upstream's hierarchical `AStar.Water*`,
  `AbstractGraph` and smoothing transformers are not ported line by line.
- **Reason:** a first plain weighted A* took up to 114 ms for ocean-crossing routes on the Large
  map (a visible hitch inside a tick); the waypoint version takes at most 1.4 ms on the same 40
  test routes.

## D36. Simplified trade and trains
- **Date:** 2026-11-20
- **Choice:** ports launch a trade ship about every 30 s (ours; upstream uses a spawn-rate
  formula) to a random other player's port on the same water; gold uses upstream `tradeShipGold`.
  Factories pay `trainGold("self")` per connected own city/port every 10 s instead of simulating
  rail networks and moving trains; rails are only drawn.
- **Reason:** the gold flows and their sizes match upstream closely enough for single player;
  full rail networks (`RailNetworkImpl`, `TrainExecution`) are a large port for little gain now.

## D37. Bots do not cross oceans
- **Date:** 2026-11-20
- **Choice:** as upstream's tribes, bots only send boats to neighbours across narrow water and to
  free land 5 tiles away. Bots-only matches where survivors sit on different continents still end
  at the 170-minute limit (seed 1); AI nations (P10) will cross oceans.

## D38. Frame-time guards for mass changes
- **Date:** 2026-11-20
- **Choice:** above 50 000 changed tiles in one frame the renderer repaints the whole map once;
  capture flashes are capped at ~20 000 per frame; bots sample at most ~150 coast tiles per side
  when picking a boat target.
- **Reason:** removed 70-100 ms frames seen in 1080p runs during mass annexations and boat
  planning by large bots.

## D39. Simplified weapons
- **Date:** 2026-11-20
- **Choice:** upstream's numbers are kept (costs, blast radii and the "inner + half of the outer
  ring" pattern, `nukeDeathFactor`, SAM range formula, silo and SAM cooldowns, warship health,
  shell damage 200-275, ranges). Simplified: missiles fly straight (upstream: parabolas, only
  drawn differently); SAMs hit instantly instead of firing a SAM missile that pre-aims; MIRV
  warheads pick random tiles of the target player within 250 tiles (upstream stages targets);
  SAMs ignore MIRVs and warheads; warship shells hit instantly within 40 tiles; trade ships are
  captured within 5 tiles; no veterancy or port healing. Water-converting nukes (a game option
  upstream) are not ported; nukes leave fallout as upstream's default.
- **Reason:** the effects on the game (what gets destroyed, how many troops die, what can be
  stopped) match upstream; the omitted parts are timing and presentation detail.

## D40. Simplified diplomacy and nation AI
- **Date:** 2026-11-20
- **Choice:** alliances, requests, renewal, traitors, relations, embargoes, donations, targets
  and their timings and numbers follow upstream (`AllianceImpl`, `PlayerImpl`, `GameImpl`,
  `execution/alliance/*`, `Donate*`, `Embargo*`, `TargetPlayerExecution`). The nation AI keeps
  upstream's structure and difficulty numbers (attack rate, reserve and trigger ratios, the
  strategy order per difficulty, alliance decisions, structure ratios per city) but reduces the
  long tails: no beachhead follow-ups, nuke planning around SAM coverage, warship hunting, AFK or
  crown logic, or chatter. Reactions are a small set of our own words instead of upstream's emoji
  table (the font has no emoji). Nation names come from our own syllable generator. No teams
  until P12, so "friendly" means allied. Also fixed: "borders unowned land" counted water, so
  coastal AIs kept sending empty land attacks instead of boats.
- **Reason:** the parts that shape a match (who allies, who betrays, who gets attacked, how hard
  nations are) match upstream; the omitted parts are refinements that need the real maps (P11)
  to tune.

## D41. Maps: styles, rivers, real Earth
- **Date:** 2026-11-21
- **Choice:** the generator gets four styles, rivers (downhill walks carved as water), impassable
  peaks and a biome layer used only for colours. Real-Earth maps are Natural Earth 1:50m land and
  lakes (public domain) rasterised by our own `earth_import` tool into run-length land masks built
  into the binary; heights and moisture are our noise (no elevation or climate data), with an
  Earth-like dry belt so deserts land roughly right. Region names do not seed nation names (the
  syllable generator stays). The default map keeps its game id, but rivers and peaks change every
  generated map, so the bots-only conquest test moved from seed 3 to seed 7.
- **Also:** ship routing now uses a coarse graph of connected water pieces per 8x8 cell (as
  upstream's abstract graph) instead of "cells with any water": the worst World-map route went from
  40 ms to 1.5 ms. Idle warships wait 20 ticks after a failed route and try a straight line first;
  nations keep at most 8 warships, send them after enemy warships, and a nation with over half the
  land nukes the biggest holdout regardless of relations (ours, to end island stalemates).
- **Reason:** real places and varied shapes without shipping OpenFront's map files (D20); the
  router and AI changes keep big and island maps fast and finishable.

## D42. Our own unit icons and smooth motion
- **Date:** 2026-11-21
- **Choice:** unit icons are generated at startup from simple shapes (`render/icons.rs`,
  supersampled, white with a dark outline) and shown as sprites; buildings get a badge tinted a
  darker shade of the owner's colour. Moving units are drawn between their tile before and after
  the last tick using the fixed-step overstep fraction (the display lags the game by at most one
  tick, 0.1 s). Zoom eases in log space.
- **Reason:** play-test feedback (line icons looked unfinished, ships stuttered at 10 Hz); generated
  art keeps D20 (no OpenFront assets) and needs no downloads.

## D43. Saves replay the match
- **Date:** 2026-11-23
- **Choice:** a save holds the match setup, the human's intents per tick, the tick count, the
  state hash and a hash of the rules. Loading builds the match and replays it; the state hash
  must match. Upstream instead snapshots every object (`snapshot/*`).
- **Reason:** the engine is deterministic, so a replay is exact, the files are tiny and no
  serializer is needed for every execution's private state. Cost: loading a long match takes a few
  seconds (the engine runs about 3 000-6 000 ticks per second).

## D44. Generated sounds; omitted modes
- **Date:** 2026-11-23
- **Choice:** sound effects are synthesised at startup (sine, square and filtered noise with
  envelopes) and played through a custom Bevy audio source; no sound files, no new crates. There is
  no music. Not ported: Doomsday Clock mode and the setup presets (Default, Fast, Long, Teams);
  every setting they would change is in the setup screen.
- **Reason:** no downloads or licences to track; the modes left out are optional variants.

## D45. Boats land anywhere on a reachable player
- **Date:** 2026-11-24
- **Choice:** a boat aimed at a player's land lands on that player's coast nearest the click
  (any distance, on a sea the attacker's coast touches); upstream gives up beyond 50 tiles. Free
  land searches up to 200 tiles. Right click opens one action menu for every kind of tile (attack,
  boat, build, warship, diplomacy), like upstream's radial menu, as a list.
- **Reason:** play-test report: most clicks on enemies across the sea did nothing.
