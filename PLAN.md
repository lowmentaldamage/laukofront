# PLAN — laukofront

laukofront is a Rust + Bevy port of the **OpenFront** engine (AGPL-3.0, see `NOTICE.md`).
This plan replaces the original territory-graph design (Stages 4–7, kept in git history). The
build-guide workflow is unchanged: finish, test, document, commit, one milestone at a time.

Upstream reference: `reference/openfrontio/` (git-ignored sparse clone, commit `129d4b32`).
Every number below comes from upstream unless marked **[ours]**.

---

## 1. Game summary

A real-time territorial strategy game on a tile map of land and water. Each player picks a spawn
point, then grows by sending a share of their troop pool into neighbouring land. Territory is
conquered tile by tile along the attack front. Troops regrow toward a cap that rises with
territory size, and gold accrues every tick. A player wins by owning 80% of the land.

## 2. Player experience

**First 5 minutes.** Pick map size, number of bots, seed. The spawn phase (100 ticks = 10 s) lets
you click a spawn tile; bots spawn randomly. Then you click unowned land next to your border to
send 20% of your troops (adjustable). Your border flows outward tile by tile. Bots expand too, and
borders start touching within a minute or two.
**Full match.** Attack bots and other players along shared borders, balancing troop regrowth
(fastest around half the cap) against expansion. The match ends when someone owns 80% of land.

## 3. Core rules (ported)

| Rule | Value | Upstream |
|---|---|---|
| Tick | 100 ms (10 Hz) | `turnIntervalMs` |
| Spawn phase | 100 ticks (single player) | `Config.numSpawnPhaseTurns` |
| Spawn immunity | 50 ticks after spawn phase | `DEFAULT_SPAWN_IMMUNITY_TICKS` |
| Start troops | human 25 000, bot 10 000 | `EngineConfig.startManpower` |
| Troop cap | `2 × (tiles^0.6 × 1000 + 50 000)`; bots ÷ 3 | `Config.troopCap` |
| Troop growth per tick | `(10 + troops^0.73 / 4) × (1 − troops / cap)`; bots × 0.5 | `Config.troopIncreaseRate` |
| Gold per tick | human 100, bot 50 | `EngineConfig.goldAdditionRate` |
| Default attack size | 20% of troops (bots 5%) | `EngineConfig.attackAmount` |
| Terrain | magnitude < 10 Plains, < 20 Highland, < 31 Mountain, 31 Impassable | `GameMapImpl.terrainType` |
| Tile attack base (mag, cost) | Plains 80/16.5, Highland 100/20, Mountain 120/25 | `terrainAttackBase` |
| Attack resolution | `EngineConfig.attackLogic` (troop ratio, defender density, large-territory bonuses) | |
| Unowned land | attacker loss `mag / 5` (bots `/10`), cost `clamp(2000 × cost / troops, 5, 100) / (2 × border)` | |
| Win | own ≥ 80% of land tiles | `PERCENT_TILES_OWNED_TO_WIN` |

Players: `Human`, `Bot` (upstream "tribes"); `Nation` AI comes in a later milestone.
Deterministic maths: all `pow`/`exp`/`log` go through a port of upstream `DetMath`.

## 4. Map **[ours]**

Upstream ships hand-made maps (CC BY-SA assets); laukofront generates its own.

| Size | Tiles | Land target |
|---|---|---|
| Small | 800 × 500 (400 k) | ~55% |
| Medium | 1200 × 750 (900 k) | ~55% |
| Large | 1600 × 1000 (1.6 M) | ~55% |

- Fractal value noise (integer hash + DetMath, seeded) gives elevation; a falloff toward the map
  edge makes continents. Sea level picks the land share; land elevation maps to magnitude 0–30.
- Tile byte uses the upstream layout: bit 7 land, bit 6 shoreline, bit 5 ocean, bits 0–4 magnitude.
  Water connected to the map edge is ocean, enclosed water is lake.
- Same seed and size → identical map.

## 5. Architecture

```
engine_lib (rng, detmath, tile map, terrain, config)   ← ports of packages/engine-lib
   └─ engine (game state, players, executions, attacks, bots, runner) ← packages/engine
        └─ game glue (Bevy resource, fixed tick)
             └─ render / input / ui / menu (Bevy)
```

- `engine_lib` and `engine` stay free of Bevy, `unwrap()` and wall-clock time.
- Upstream's Intent → Execution model is kept: input and bots create intents; the runner turns them
  into executions; `tick()` runs every active execution, then per-player economy.
- Rendering reads per-tick "changed tiles" from the engine and writes them into a GPU texture.

## 6. Data model (first milestones)

- `TileRef = u32` (index `y * width + x`); `GameMap { width, height, terrain: Vec<u8>, state: Vec<u16> }`
  where `state` holds owner small-id (12 bits) + flags, as upstream.
- `Player { small_id, kind, name, troops: f64, gold: i64, tiles: u32, border: TileSet, spawned, alive }`.
- `Game { map, players, ticks, executions, rng, config }`.
- `Execution` trait: `init`, `tick`, `is_active`, `active_during_spawn_phase`.
- `AttackExecution { owner, target, troops, to_conquer: heap, border_size }`.

## 7. Performance budget

Unchanged targets: 60 FPS at 1080p on a mid-range PC; tick < 2 ms average at 10 Hz; no frame over
33 ms. Measured on Medium (900 k tiles) with 1 human + 10 bots.

## 8. Rendering

One `R16Uint`/RGBA texture with one texel per tile: terrain colour + owner tint; per tick only
changed tiles are written. Borders drawn by the fragment shader (neighbour owner differs).
Camera from the Stage 8 work (pan, zoom to cursor, clamped). Our own palette.

## 9. UI

Top bar: troops / cap, gold, tiles %, timer. Attack-ratio slider (keys 1–9 = 10–90%). Leaderboard.
Main menu shows "© OpenFront and Contributors" (licence requirement).

## 10. AI

Milestone P4: upstream `TribeExecution` + `AiAttackBehavior` (bots). Later: `NationExecution`.

## 11–13. Save/load, audio, multiplayer

Later milestones. Upstream has snapshots and a relay server; not in the core loop.

## 14. Testing strategy

- Port upstream engine tests that cover ported code (`tests/` in upstream, by name).
- Our own: determinism (same seed + intents → same state hash), invariants (tile owner counts
  match player tile counts), formula tests against values computed from the upstream formulas.

## 15. Packaging

Unchanged (`docs/RELEASE.md`), plus `LICENSE` and `NOTICE.md` in the zip.

## 16. Milestones

| # | Milestone | Accepted when |
|---|---|---|
| P0 | Rename, relicense, plan | builds as `laukofront`; LICENSE/NOTICE in place |
| P1 | Engine lib | sfc32 matches upstream values; DetMath tests; generated maps deterministic |
| P2 | Players + economy + spawn | growth/cap/gold match formulas; spawn claims tiles |
| P3 | Attacks + win | tile-by-tile conquest; retreat; 80% win; invariants hold over long runs |
| P4 | Bots | bots-only match reaches a winner |
| P5 | Bevy client | map renders from texture; click to spawn/attack; HUD |
| later | boats, structures, alliances, nations, nukes, warships, trade | one per milestone |

## 17. Risks

| Risk | Fallback |
|---|---|
| Upstream is large and changes daily | port from a pinned commit; record it in NOTICE |
| f64 maths differences | port DetMath exactly; avoid platform `powf` in the engine |
| 1.6 M-tile maps too slow | profile; border-only data structures as upstream |
| Licence compliance | NOTICE + visible notice in menu; AGPL source offer with releases |

## 18. Glossary

Tile, TileRef, magnitude, terra nullius (unowned land), small id, execution, intent, border tile,
spawn phase, bot (tribe), nation.
