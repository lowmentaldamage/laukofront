# Architecture

laukofront ports the OpenFront engine (see `NOTICE.md`). Module names follow upstream packages.

## Modules

| Module | Ports (upstream) | Owns | Bevy? |
|---|---|---|---|
| `engine_lib::random` | `engine-lib/PseudoRandom.ts` | sfc32 PRNG, bit-identical to upstream | no |
| `engine_lib::detmath` | `engine-lib/DetMath.ts` | deterministic `exp`/`log`/`pow`/`atan2` | no |
| `engine_lib::game_map` | `engine-lib/game/GameMapImpl.ts` | tile grid: terrain byte + owner state per tile | no |
| `engine_lib::terrain` | — (ours) | map generator: styles, rivers, peaks, biomes | no |
| `menu` | — (ours) | main menu, match setup, settings, credits, pause menu | yes |
| `client::save` | snapshot (replaced by replay saves, D43) | save files | no |
| `client::audio` | — (ours) | generated sound effects (D44) | yes |
| `render::icons` | — (ours) | generated icon textures for buildings and ships (D42) | no |
| `engine_lib::earth` | — (ours) | real-Earth maps: mask format, built-in files, map building | no |
| `config` | parts of `Config.ts` / `EngineConfig.ts` | `GameConfig` loaded from `assets/config/game.toml` | no |
| `engine::player` | `game/PlayerImpl.ts` (data) | `Player`, `PlayerInfo`, `PlayerType` | no |
| `engine::rules` | `Config.troopCap`, `troopIncreaseRate`, `goldAdditionRate`, `startManpower`, `attackAmount` | formulas | no |
| `engine::game` | `game/GameImpl.ts` (core) | `Game`: map, players, tick loop, conquer/relinquish, borders, spawn phase; `Execution` trait | no |
| `engine::executions` | `SpawnTimerExecution`, `SpawnExecution`, `PlayerExecution` | executions | no |
| `engine::attack` | `game/AttackImpl.ts`, `utils/FlatBinaryHeap.ts` | `Attack`, `TileHeap` | no |
| `engine::attack_exec` | `AttackExecution`, `RetreatExecution`, `WinCheckExecution` | attacks, retreat, win | no |
| `engine::clusters` | `PlayerExecution.removeClusters` (simplified, D25) | encirclement | no |
| `engine::bots` | `TribeExecution`, `TribeAttackBehavior`, `AiAttackBehavior` (land) | bot AI | no |
| `engine::units` | `UnitImpl` (parts) | `Unit`, `UnitKind` | no |
| `engine::water` | pathfinding, abstract graph (simplified, D35, D41) | water bodies, coarse water graph, ship routes | no |
| `engine::boats` | `TransportShipExecution`, `BoatRetreatExecution`, `TransportShipUtils`, trade (D36) | ships | no |
| `engine::structures` | `ConstructionExecution`, `UpgradeStructureExecution`, unit costs, factories (D36) | buildings | no |
| `engine::weapons` | `NukeExecution`, `MIRVExecution`, `SAMLauncherExecution`, `WarshipExecution`, `MoveWarshipExecution` (simplified, D39) | missiles, SAMs, warships | no |
| `engine::diplomacy` | `AllianceImpl`, `AllianceRequestImpl`, alliance / donate / embargo / target / emoji executions (D40) | alliances, traitors, relations, embargoes, donations | no |
| `engine::nations` | `NationExecution`, `nation/*Behavior` (reduced, D40) | AI nations | no |
| `engine::runner` | `GameRunner.ts`, `ExecutionManager.ts` | match setup, `Intent` → execution | no |
| `engine::util` | `Util.simpleHash`, `getSpawnTiles` | helpers | no |
| `client` | — | `GameState` resource, fixed-tick runner, pending intents, changed tiles, settings, notices | yes |
| `render` (+ `camera`, `palette`, `perf`) | — | map texture, repaint of changed tiles, camera, colours, frame-time/screenshot tools | yes |
| `input` | — | click rules (`click_action`, pure), mouse and keys → intents | yes |
| `ui` (`hud`, `minimap`, `tutorial`, `labels`, `endscreen`) | — | HUD, minimap, tutorial, map labels, end screen; pure number/row builders in `ui/mod.rs` | yes |
| `render::effects` | — | capture flash overlay, attack-front shimmer, spawn rings | yes |
| `client::settings` | — | settings file | yes |
| `menu` | — | Stage 12 | yes |
| `bin/engine_bench`, `bin/map_preview`, `bin/bot_match`, `bin/earth_import` | — | tools (`earth_import` turns Natural Earth shapefiles into `assets/maps/*.lkmap`) | no |

## Allowed dependencies

```
engine_lib  ←  config  ←  engine  ←  (Bevy) client  ←  render / input / ui / menu  ←  main.rs
```

- Nothing in `engine_lib`, `config` or `engine` uses Bevy, wall-clock time, `unwrap()` outside
  tests, or platform transcendental maths (`powf`, `exp`, `ln` → use `detmath`).
- The client only reads `Game` and sends `Intent`s; it never mutates game state directly.

## Tick (`engine::game::Game::execute_next_tick`, as upstream)

1. Clear last tick's changed-tile list.
2. Tick every active execution (during the spawn phase, only those active in it).
3. Initialise executions added this tick, FIFO, including ones added while initialising. During the
   spawn phase, executions not active in it stay queued.
4. Drop finished executions.
5. `ticks += 1`.

So an intent sent for tick N creates an execution that is initialised at the end of N and acts on
N + 1 — the same one-tick delay as upstream.

## Ownership bookkeeping

The tile map stores each tile's owner (12-bit small id, 0 = nobody). Each `Player` keeps vectors of
its tiles and border tiles; the `Game` keeps, per tile, its index in its owner's vectors, so add and
remove are O(1). `conquer`/`relinquish` update the four neighbours' border status, as upstream.

## Data flow

```
input / bots ─► Intent ─► run_turn ─► Execution ─► Game::execute_next_tick ─► changed tiles ─► renderer
```
