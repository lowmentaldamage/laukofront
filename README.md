# laukofront

A real-time territorial strategy game in Rust + Bevy: spawn on a generated map, push your borders
tile by tile, balance troop growth against expansion, and own 80% of the land to win.

laukofront is a Rust port of the **OpenFront** engine (<https://github.com/openfrontio/openfrontio>).
**© OpenFront and Contributors.** laukofront is not affiliated with or endorsed by OpenFront.
See `NOTICE.md`.

**Status:** playable against bots (port milestones P0–P5). Boats, menus and polish are next.
See `PROJECT_STATUS.md`. See `PROJECT_STATUS.md`.

## Quick start (Windows, PowerShell)

```powershell
cargo run --release                       # play: Medium map, 20 bots
cargo run --release -- --size large --bots 50 --seed 7
cargo run --release -- --nations 8 --difficulty hard
cargo run --release -- --autoplay         # watch it play itself
cargo test
```

Click unclaimed land to spawn, then click land next to your border to attack. Keys 1-9 set the
attack size, R retreats, Space pauses, F1-F3 change speed. Full list: `docs/CONTROLS.md`.

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
