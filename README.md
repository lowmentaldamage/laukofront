# openfront_clone

An OpenFront-style real-time territorial conquest game written in Rust with Bevy: grow troops,
earn gold, and attack neighbouring territories until you are the last player standing.

**Status:** the simulation core and benchmark are done (build-guide Stages 0–7). The game is not
playable yet. See `PROJECT_STATUS.md`.

## Quick start (Windows, PowerShell)

```powershell
cargo test                              # run the simulation tests
cargo run --release --bin sim_bench     # headless benchmark
cargo run                               # the game (currently an empty app that exits)
```

Full setup from a clean PC: `docs/BUILDING.md`. Smart App Control must be off to build.

## Documentation

| | |
|---|---|
| `PLAN.md` | the full plan: rules, numbers, architecture, milestones |
| `docs/GAMEPLAY.md` | every rule and number |
| `docs/ARCHITECTURE.md` | modules and dependencies |
| `docs/MAP_GENERATION.md` | how maps are made |
| `docs/CONTROLS.md` | keys and mouse |
| `docs/TESTING.md`, `docs/PERFORMANCE.md` | test and benchmark results |
| `DECISIONS.md` | why things are the way they are |
| `docs/CREDITS.md` | crates and assets with licenses |

## License

MIT — see `LICENSE.md`.
