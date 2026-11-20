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
