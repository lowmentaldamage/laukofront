# laukofront

A real-time territorial strategy game in Rust + Bevy: spawn on a generated map, push your borders
tile by tile, balance troop growth against expansion, and own 80% of the land to win.

laukofront is a Rust port of the **OpenFront** engine (<https://github.com/openfrontio/openfrontio>).
**© OpenFront and Contributors.** laukofront is not affiliated with or endorsed by OpenFront.
See `NOTICE.md`.

**Status:** porting in progress — milestone P0 (rename, relicense, plan) done; the engine port
starts with P1. Not playable yet. See `PROJECT_STATUS.md`.

## Quick start (Windows, PowerShell)

```powershell
cargo test
cargo run --release --bin sim_bench
```

Full setup from a clean PC: `docs/BUILDING.md`. Smart App Control must be off to build.

## Documentation

| | |
|---|---|
| `PLAN.md` | the port plan: rules, map, architecture, milestones |
| `NOTICE.md` | origin, attribution, licence terms |
| `DECISIONS.md` | why things are the way they are |
| `docs/` | gameplay, architecture, testing, performance, building, release |

## License

GNU AGPL-3.0-only with OpenFront's additional terms — see `LICENSE` and `NOTICE.md`.
