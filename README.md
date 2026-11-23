# laukofront

A real-time territorial strategy game in Rust + Bevy: spawn on a generated map, push your borders
tile by tile, balance troop growth against expansion, and own 80% of the land to win.

laukofront is a Rust port of the **OpenFront** engine (<https://github.com/openfrontio/openfrontio>).
**© OpenFront and Contributors.** laukofront is not affiliated with or endorsed by OpenFront.
See `NOTICE.md`.

**Status:** version 1.0.0, a complete single-player game: menus, AI nations and bots, diplomacy,
teams, boats, buildings, warships, nukes, generated and real-Earth maps, saves and sound. See
`PROJECT_STATUS.md`.

## Quick start (Windows, PowerShell)

```powershell
cargo run --release                       # main menu
cargo run --release -- --size large --bots 50 --seed 7
cargo run --release -- --nations 8 --difficulty hard
cargo run --release -- --map world          # or europe, asia, archipelago… (--list-maps)
cargo run --release -- --autoplay         # watch it play itself
cargo test
```

Play -> pick a map -> Start. Click unclaimed land to spawn, then click land next to your border
to attack. Keys 1-9 set the attack size, Esc opens the pause menu (save, load, settings). Full
list: `docs/CONTROLS.md`. A ready-to-run Windows zip: `scripts\package.ps1` (see
`docs/RELEASE.md`).

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
