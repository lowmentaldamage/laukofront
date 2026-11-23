# Testing

## How to run

```powershell
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Tests read `assets/config/game.toml`, so run them from the project root (cargo does this).

## What is tested

| File | Covers |
|---|---|
| `src/core/rng.rs` (unit) | SplitMix64 reference value; ranges stay in bounds |
| `src/config/mod.rs` (unit) | shipped files load; missing key named; unknown key rejected; out-of-range value rejected; missing file names its path |
| `tests/map_tests.rs` | neighbour symmetry (3 sizes × 4 seeds); sorted/unique/no-self lists; same seed → same map; different seed → different map; counts match config; every territory connected; graph connected; out-of-bounds lookup; dense and over-full maps |
| `tests/combat_tests.rs` | win, lose, tie, capture (incl. empty neutral); full attack through `World` resolving on the next tick; non-neighbour refused; sending every troop refused |
| `tests/economy_tests.rs` | growth formula; growth stops at cap; growth only every 10th tick; above-cap troops kept; neutrals never grow; cap rises with owned neighbours; 60 gold per territory per minute; gold scales with territories; reinforce cost and cap clamp; reinforce without gold refused |
| `tests/sim_tests.rs` | 500-tick random runs keep ownership consistent; determinism with same seed and orders; start state; neutral troop range; setup errors; every invalid-order case; stale orders dropped; elimination and conquest; eliminated player cannot act; time-limit victory; 1-player match never ends by conquest |

Required by the guide (Stage 6), all covered: neighbour symmetry, same seed same map,
combat win/lose/tie/capture, growth stops at cap, gold at configured rate, 500-tick run keeps
exactly one owner per territory.

## Play-test checklist (Stage 16)

To be filled with date, machine and result for each line:

- [ ] Start the game from the built `.exe`, not from `cargo run`
- [ ] Play one full match against 3 AIs
- [ ] Try an invalid attack (non-neighbour, no troops)
- [ ] Pause, speed change, restart
- [ ] Change a setting and check it persists
- [ ] Resize the window and check the UI
- [ ] Run for 10 minutes with no crash

## Latest results

Results are added below as each stage runs them.

### 2026-11-20 — Stage 6 (dev machine, see ENVIRONMENT.md)

| Suite | Passed | Failed |
|---|---|---|
| unit tests in `src/` (rng, config) | 7 | 0 |
| `tests/combat_tests.rs` | 7 | 0 |
| `tests/economy_tests.rs` | 10 | 0 |
| `tests/map_tests.rs` | 10 | 0 |
| `tests/sim_tests.rs` | 11 | 0 |
| **Total** | **45** | **0** |

- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo fmt --check`: clean.
- No `unwrap()`/`expect()` in `src/` outside `#[cfg(test)]`; no Bevy imports in the simulation
  modules (checked with grep).

### 2026-11-20 — Port milestone P1 (engine_lib)

- `engine_lib::random`: sfc32 stream matches upstream bit for bit for 6 seeds (incl. negative and
  > 32-bit seeds); state round-trip; shuffle is a permutation.
- `engine_lib::detmath`: `exp`, `log`, `pow`, `atan2` match upstream bit patterns; within 1e-8 of std.
- `engine_lib::game_map`: neighbour order N,S,W,E; terrain types; owner ids; border; edges.
- `engine_lib::terrain`: determinism, land share, ocean edge, shoreline consistency, terrain mix.
- Reference values: hand transcription of upstream PseudoRandom/DetMath run in Node 26 (D21).

### 2026-11-20 — Port milestone P2 (players, spawn, economy)

The Stage 6 test files were removed with the old sim (D24); the counts above are history.

| Suite | Passed |
|---|---|
| unit tests in `src/` (engine_lib, config, engine::player/rules/util) | 33 |
| `tests/engine_p2.rs` | 10 |
| **Total** | **43**, 0 failed |

Covers: bot spawns (full 52-tile disks, ≥ 30 apart), spawn phase timer (ends on tick 101), human
spawn ends the phase, re-spawn moves the player, late spawn ignored, spawn on water/coast, no growth
during spawn phase, growth and gold equal the upstream formulas tick by tick for 50 ticks, troops
level off at the cap, determinism (state hash), conquer/relinquish bookkeeping (tile and border sets
always match the map). Upstream `simpleHash` checked against Java string hashes.
`cargo clippy --all-targets -- -D warnings`: clean.

### 2026-11-20 — Port milestone P3 (attacks)

| Suite | Passed |
|---|---|
| unit tests in `src/` | 36 |
| `tests/engine_p2.rs` | 10 |
| `tests/engine_p3.rs` | 16 |
| **Total** | **62**, 0 failed |

P3 covers: `attackLogic` equals the upstream formula for a player defender and for unowned land
(plus bot loss and water rejection); attacks on unowned land expand and cost troops; the default
attack sends 20%; attacks on a bot take land and kill troops; an attack with no shared border is
refunded; spawn immunity (humans blocked, bots not); retreat freezes the attack and refunds
exactly growth + 75% after ~20 ticks; opposing attacks cancel; same-target attacks merge; a
< 100-tile defender is wiped out and loses its gold; an encircled pocket is annexed, one touching
unowned land is not; 80% wins, 70% does not; a 13-player, 600-tick random-attack match keeps every
invariant and is deterministic. The tile heap pops in priority order with `f32` priorities.

### 2026-11-20 — Port milestone P4 (bots)

| Suite | Passed |
|---|---|
| unit tests in `src/` | 36 |
| `tests/engine_p2.rs` | 10 |
| `tests/engine_p3.rs` | 16 |
| `tests/engine_p4.rs` | 4 |
| **Total** | **66**, 0 failed |

P4 covers: a 10-bot match (seed 3) reaches a winner by conquest (> 80% land, well before the time
limit); bots expand past their spawn disks and later attack each other; every new bot attack
targets a land neighbour and never carries more troops than the bot had; bots-only matches are
deterministic. The P2 economy test now checks troops for the human only (bots spend troops).

Bots-only matches (`cargo run --release --bin bot_match -- --size small --bots 10 --seed N`):

| Seed | Result |
|---|---|
| 1 | stalled at 62% (2 bots on separate landmasses) → time-limit win at 170 min |
| 2 | Bot 9 wins at 45.7 min |
| 3 | Bot 9 wins at 36.5 min |
| 4 | Bot 7 wins at 62.7 min |
| 5 | still 4 alive at 66 min (water) |
| 6 | Bot 5 wins at 32.3 min |
| 7 | Bot 4 wins at 43.1 min |
| 8 | Bot 5 wins at 63.5 min |

### 2026-11-20 — Port milestone P5 (Bevy client)

| Suite | Passed |
|---|---|
| unit tests in `src/` | 50 |
| `tests/engine_p2.rs` | 10 |
| `tests/engine_p3.rs` | 16 |
| `tests/engine_p4.rs` | 4 |
| **Total** | **80**, 0 failed |

New unit tests: tile ↔ world round trip (picking); player colours distinct and never close to the
human's blue (RGB distance ≥ 90 for 300 ids); owned tiles tinted, borders darker; click rules
(spawn on free land only during the spawn phase; afterwards attack free land next to you with
`floor(troops × size)`, refuse own land); HUD numbers equal the game state (troops, attack size,
leaderboard count and leader); number formatting.

Manual check (2026-11-20, dev PC): `laukofront --size medium --bots 20 --seed 3 --autoplay` —
map, territories, borders, HUD, leaderboard and attribution visible; the autoplayed human spawns
and attacks; screenshot reviewed. A human mouse play-test is part of Stage 16.

### 2026-11-20 — Port milestone P6 (interface)

| Suite | Passed |
|---|---|
| unit tests in `src/` | 61 |
| `tests/engine_p2.rs` / `p3` / `p4` | 10 / 16 / 4 |
| **Total** | **91**, 0 failed |

New tests: HUD numbers equal the game (troops, cap, growth/s, gold, gold/s = 1000, tiles, win
share); leaderboard sorted and always includes you; attack lines list outgoing attacks; number
format; notifications for eliminations during a bots match; notice list keeps the newest six;
saved settings round trip, clamping and broken files; tutorial steps advance only in order; labels
sit on their owner's land and grow with territory; minimap size, pixels and camera rectangle;
capture flash fades to zero; full repaint has one texel per tile.

Manual (2026-11-20, dev PC): screenshots after spawn and at 2 minutes reviewed — left HUD, minimap,
clock/speed buttons, gold border and YOU marker, tutorial bubble, labels; fixed leaderboard rows
wrapping, oversized labels, and see-through panels.

### 2026-11-20 — Port milestones P7 + P8 (boats, buildings)

| Suite | Passed |
|---|---|
| unit tests in `src/` | 67 |
| `tests/engine_p2.rs` / `p3` / `p4` | 10 / 16 / 4 |
| `tests/engine_p7_p8.rs` | 13 |
| **Total** | **110**, 0 failed |

New: water bodies are separate; routes stay on water, pass the only gap, have no corner cuts;
no route into a lake. Boats cross the sea, land and keep attacking; at most 3 boats; no boat
without a coast or onto own land; retreating boats come home. Trade gold formula. Building costs
double and cap; cities add 250K cap only when finished; placement rules (own land, coast for
ports, 15-tile spacing, gold); buildings captured with their land, defense posts destroyed;
defense posts slow attacks; upgrades; factory gold (2 cities = 20K per run, exact); ports trade.
Client click rules: build mode, upgrade by clicking own building, boat across water (this test
found and fixed a bug: free land anywhere counted as attackable; ported upstream `canAttack`).
`bot_match` (Small, 10 bots): seeds 2, 3, 5, 6 end by conquest (seed 5 used to stall — boats).

### 2026-11-20 — Port milestone P9 (warships, missiles, fallout)

**117 passed, 0 failed** (68 unit, 10 / 16 / 4 / 13 / 6 in `engine_p2/p3/p4/p7_p8/p9`).

New: `nuke_death_factor` equals upstream for atom and MIRV; atom bomb needs a silo, costs 750K,
flies, destroys the centre, leaves fallout inside the inner radius and nothing far away, kills
troops; hydrogen bomb takes > 10 000 tiles; silo reloads after 90 ticks and level 2 fires two;
a SAM stops an atom bomb (no damage) but not a MIRV, and MIRVs get 15M dearer; fallout slows
expansion; a warship sinks a passing boat (no landing) and warships damage each other.

Manual check (later the same day, PC free): autoplay screenshot shows the left HUD with
attacks and retreat buttons, the build bar with the weapons group and prices, defense-post icons,
labels and minimap. Frame-time runs: 0 frames over 33 ms (see PERFORMANCE.md). Cosmetic issues
noted: long notices wrap onto two lines; map labels show faintly through the leaderboard panel.

### 2026-11-20 — Port milestone P10 (diplomacy and nations)

**132 passed, 0 failed** (70 unit, 10 / 16 / 4 / 13 / 6 / 13 in
`engine_p2/p3/p4/p7_p8/p9/p10`).

New: request and accept, no attacks between allies, asking back accepts and pulls back running
attacks, requests expire after 20 s with a 30 s cooldown, breaking makes a 30 s traitor who
defends worse, alliances expire unless both renew, donations only to allies with a cooldown,
embargo blocks trade, targets and reactions expire, nuking an ally breaks the alliance, allying
cancels nukes in flight, difficulty sets nation troops, a nation match ends with a nation winner
that built, nation matches are deterministic; diplomacy notices; menu actions and panel rows.

`bot_match` with 8 bots and 4 nations, seeds 1-6, every difficulty: **24 of 24 matches end with a
winner** (6-29 game minutes). Bots-only matches (no nations): seed 3 ends at 36 min, seeds 1 and 5
stall with bots on separate islands, as upstream (tribes do not cross oceans; nations do). Seed 5
ended before only because of the water-border bug fixed in D40.

Manual check: autoplay screenshot with 6 nations shows the diplomacy menu on the biggest nation
(alliance, embargo, target, reactions, info line with relation), nation names on the map and in
the leaderboard.

### 2026-11-21 — Port milestone P11 (maps)

**143 passed, 0 failed** (73 unit, 10 / 16 / 4 / 13 / 6 / 13 / 8 in
`engine_p2/p3/p4/p7_p8/p9/p10/p11`).

New: map styles (archipelago has less land and more islands, pangaea is one landmass, lakes has
many lakes), peaks impassable and not counted, rivers reach the sea, biomes follow latitude, real
places are land or sea (Sahara, Siberia, Brazil, Australia, Canada; Atlantic, Pacific, Indian Ocean;
the Caspian is a lake; England, Ireland), Earth maps build the same every time, every map name
parses and plays, a ship crosses the Atlantic in under 50 ms; mask format round trip and corrupt
files rejected. `engine_p4` conquest test moved to seed 7 (D41).

`bot_match` (8 bots + 4 nations, small): continents, pangaea and lakes seeds 1-3 all end with a
winner; archipelago seeds 1-6: 5 of 6. Earth maps (20 bots + 8 nations): all six end with a winner
(8-30 game minutes).

Manual check: World and Europe with autoplay. Coastlines, biomes, labels, alliance panel and
minimap show correctly; fixed the diplomacy menu drawing over the end screen.

### 2026-11-21 — Icons, smooth motion, smaller names

**147 passed, 0 failed** (77 unit). New: every icon has a shape and an outline and fits inside its
texture, icons all differ, every building and ship kind has one; zoom easing moves partway each
frame and arrives within a second. Screenshots (medium generated map, Europe) show ports, defense
posts, cities and ships with their icons, readable badges and smaller names.

### 2026-11-23 — Port milestones P12-P13 (menus, teams, save, sound)

**153 passed, 0 failed** (81 unit, 10 / 16 / 4 / 13 / 6 / 13 / 8 / 2 in
`engine_p2 ... engine_p12`).

New: teams are balanced and bots have none; teammates are friendly, never attack each other over a
whole match, and the winning team owns the winner; save round trip replays to the same state
hash; wrong version, wrong rules and wrong hash are refused; every sound is short, audible and in
range; menu values step, wrap and print correctly; settings round trip with the new fields.
Bots + nations in 2-4 teams (seeds 1-2): all six matches end with a team winner.

Manual check: main menu and setup screen screenshots (HUD hidden behind the menus); team match
runs (579 FPS). One 82 ms frame at 2 s into a run, when the first sound opens the audio device.

### 2026-11-23 — Release v1.0.0 (P14)

Full checklist with results: `docs/QA.md`. 153 tests pass; `--menu-script` presses the real menu
buttons (Play, Start, pause, Save game, Main menu, Continue saved game) and every step does what
it should; the packaged zip runs from an outside folder. Found and fixed: the saved VSync setting
capped the frame-time measurement at the monitor's 165 Hz.
