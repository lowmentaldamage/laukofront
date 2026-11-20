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
