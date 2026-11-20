# PLAN — openfront_clone

The mega plan (Stage 4). Every balance number here also lives in `assets/config/game.toml` or
`assets/config/ai.toml`; if the two ever disagree, the config file wins and this plan gets fixed.

---

## 1. Game summary

openfront_clone is a real-time territorial conquest game for one human against 1–5 AI opponents.
The map is a randomly generated patchwork of 80–300 territories, each owned by a player or neutral.
Owned territories grow troops and earn gold every second; neutral territories never grow.
You win by sending troops from your territories into neighbouring ones until every opponent has
lost their last territory. A match lasts 5–20 minutes and is capped at 60 minutes.

## 2. Player experience

**First 5 minutes.** The player clicks Play, keeps the default setup (Small map, 3 AIs, Normal,
random seed) and the map appears with their one blue starting territory highlighted. A hint bar
says "Click your territory, then click a neighbour to attack". The first attacks go into weak
neutral land (5–20 troops). By minute 2 the player owns ~6 territories and their border touches an
AI. Gold piles up; the player learns that `R` turns 200 gold into 25 troops. By minute 5 the
player has had a first fight with an AI and seen a capture notification on both sides.

**First full match.** Neutral land runs out around minutes 4–6. The middle game is border fights:
the player watches the troop caps (bigger when surrounded by own land), saves troops, and hits weak
spots with 50–100% attacks. AIs eliminate each other too. The match ends when one player is left
(end screen: winner, time, territories held, Restart / Menu) or at 60 minutes, when the player
with the most territories wins.

## 3. Core rules with exact numbers

| Rule | Value |
|---|---|
| Simulation tick | 10 Hz (100 ms), fixed step, independent of frame rate |
| Players | 1 human + 1–5 AI (max 6 players) |
| Starting territories | 1 per player, chosen spread apart (see §4) |
| Starting troops (player territory) | 50 |
| Neutral troops | random 5–20 per territory, never grow |
| Growth interval | every 10 ticks (1 s) |
| Growth amount | `max(1, troops × 5 / 100)` (integer), owned territories only |
| Troop cap | `100 + 20 × (number of neighbours with the same owner)` |
| Above cap | troops above the cap are kept but do not grow; growth never pushes past the cap |
| Gold | 60 gold/min per owned territory = 100 milli-gold per tick; starting gold 0 |
| Reinforce | costs 200 gold, adds 25 troops to an own territory, clamped to its cap; refused if already at/over cap |
| Attack | from an own territory to an adjacent territory not owned by the attacker |
| Troops sent | 1 … (source troops − 1); the source always keeps ≥ 1 |
| UI send presets | 25%, 50%, 100% of (source troops − 1), rounded down, minimum 1 |
| Order timing | orders queued during tick N resolve at the start of tick N+1, in queue order |
| Attacker wins (A > D) | territory captured; new troops = A − D |
| Tie (A = D) | defender troops become 0, owner unchanged |
| Attacker loses (A < D) | defender troops become D − A |
| Stale order | if, at resolution, the source is no longer owned by the sender, or the target is now the sender's, the order is dropped; troops are clamped to (source − 1) and dropped if that is 0 |
| Elimination | a player with 0 territories is eliminated and can no longer act |
| Victory: conquest | exactly one player not eliminated (only checked when the match started with ≥ 2 players) |
| Victory: time limit | at tick 36 000 (60 min): most territories, then most total troops, then lowest player id |

## 4. Map

| Size | Raster (cells) | Territories |
|---|---|---|
| Small | 192 × 120 | 80 |
| Medium | 256 × 160 | 150 |
| Large | 352 × 220 | 300 |

- **Generation:** raster Voronoi. Seed points are placed with best-candidate sampling
  (10 candidates per point, keep the one farthest from existing points). Every cell belongs to the
  nearest seed point by integer squared distance; ties go to the lower territory id. A repair pass
  makes every territory one connected blob. Two territories are neighbours when any of their cells
  touch along an edge (4-connectivity). Details: `docs/MAP_GENERATION.md`.
- **Seeds:** a `u64`. All randomness comes from one in-house SplitMix64 generator, so the same seed
  and size always give the same map on any machine. Sub-streams: map = `seed`, setup (neutral troops,
  starts) = `seed ^ 0x5EED_0F_5E70_0000`.
- **Starting positions:** first player gets a random territory; each next player gets the
  territory whose centre is farthest (min squared distance to all chosen starts), ties → lower id.

## 5. Architecture

```
core ← config ← map ← territory ← combat ─┐
                         ↑                ├─ sim ← ai ← (Bevy) render / input / ui / menu ← main
                         └──── economy ───┘
```

- Plain-Rust simulation crate modules: `core`, `config`, `map`, `territory`, `combat`, `economy`,
  `sim`. No Bevy types, no `unwrap()`, no allocation in the per-tick loop.
- Bevy modules (`render`, `input`, `ui`, `menu`) read the `World` through a Bevy resource and only
  change it through `World::queue_order`. `ai` produces the same orders a human can.
- **Tick vs frame:** Bevy's `FixedUpdate` runs `World::tick()` at 10 Hz × game speed (1×/2×/3×).
  Rendering runs every frame and only updates mesh colours for territories whose owner changed
  (from the tick's event list).
- Data flow per tick: input/AI → `queue_order` → `tick()` → `events` → render/UI/audio.

## 6. Data model

```rust
pub struct TerritoryId(pub u16);
pub struct PlayerId(pub u8);
pub enum Owner { Neutral, Player(PlayerId) }

pub struct Map {
    width: u16, height: u16,
    cells: Vec<u16>,               // width*height, territory index per cell
    seeds: Vec<(u16, u16)>,        // seed point per territory
    area: Vec<u32>,                // cell count per territory
    centroid_sum: Vec<(u64, u64)>, // for centre-of-mass
    neighbor_offsets: Vec<u32>,    // CSR: neighbours of t = neighbor_list[off[t]..off[t+1]]
    neighbor_list: Vec<u16>,       // sorted, deduplicated, symmetric
}

pub struct Territory { owner: Owner, troops: u32, troop_cap: u32, gold_per_min: u32 }

pub struct PlayerState { id: PlayerId, gold_milli: u64, alive: bool, territories: u32, troops: u64 }

pub enum Order {
    Attack { player: PlayerId, from: TerritoryId, to: TerritoryId, troops: u32 },
    Reinforce { player: PlayerId, territory: TerritoryId },
}

pub enum AttackOutcome { Captured { remaining: u32 }, Tie, Repelled { defender_left: u32 } }

pub enum Event {
    Attack { player, from, to, defender: Owner, sent, outcome },
    Reinforced { player, territory, added },
    OrderDropped { order },
    Eliminated { player },
    GameOver { winner: PlayerId, reason: VictoryReason }, // Conquest | TimeLimit
}

pub struct World {
    config: GameConfig, map: Map, territories: Vec<Territory>, players: Vec<PlayerState>,
    tick: u64, winner: Option<(PlayerId, VictoryReason)>,
    orders: Vec<Order>, resolving: Vec<Order>, events: Vec<Event>,
}
```

## 7. Performance budget

| Target (do not change to pass) | Value |
|---|---|
| Frame rate | 60 FPS at 1080p on a mid-range PC, 80 territories, 3 AIs |
| Sim tick | < 2 ms at 10 Hz |
| Worst frame | no frame over 33 ms in a 5-minute run |
| Headless sim speed | ≥ 100 ticks/s (10× real time) for 80 territories + 3 AIs |

Per-frame budget (16.6 ms): sim ≤ 2 ms (amortised ≤ 0.2 ms/frame at 10 Hz), AI ≤ 1 ms per
think, render ≤ 6 ms, UI ≤ 2 ms, rest is headroom. Map generation ≤ 500 ms for Large.

## 8. Rendering

- One combined mesh: one quad per raster row-run of equal territory id; each vertex carries the
  territory colour. Owner change → rewrite only that territory's vertex colours.
- Colours: neutral `#5A5F66`; players blue `#3B82F6` (human), red `#EF4444`, green `#22C55E`,
  amber `#F59E0B`, purple `#A855F7`, cyan `#06B6D4`.
- Borders: a separate line mesh along cell edges between different territories, 1 px dark
  (`#111418`, 60% alpha); thicker 2 px between different owners.
- Selection: selected territory brightened +35%; valid attack targets outlined white; hover +15%.
- Camera: pan with WASD/arrows or right-drag, zoom with wheel (0.5×–4×), clamped to map bounds.
- Picking: mouse → world → cell → `cells[]` lookup (O(1)).

## 9. UI

- Top bar: gold, total troops, territory count, income/min, match timer (mm:ss), speed.
- Left panel: players with colour swatch, territory count, eliminated players greyed.
- Right panel (territory selected): owner, troops / cap, gold/min, neighbour count, Attack (25/50/100) and Reinforce buttons.
- Bottom: controls hint and the last 5 notifications (5 s each).
- Style: dark translucent panels (`#0B0E13` at 80%), white numbers, 16 px font.
- UI reads `World`; it changes nothing directly.

## 10. AI

- Thinks every 10 ticks (1 s), staggered by player id so AIs do not all think on one tick.
- Decision: (1) attack the weakest neutral neighbour it can capture, (2) else attack the weakest
  enemy neighbour when `sent ≥ ratio × defender`, (3) else reinforce if gold ≥ 200 and a border
  territory is below cap, (4) else hold.
- Difficulty (`ai.toml`): Easy — attack ratio 2.0, send 50%, thinks every 20 ticks;
  Normal — ratio 1.3, send 60%, every 10 ticks; Hard — ratio 1.15, send 80%, every 10 ticks, prefers
  targets that raise its own troop caps and keeps a 30-troop reserve on threatened borders.
- AI uses `World::queue_order` only.

## 11. Save/load (Stage 13, optional)

TOML file with `version = 1`, seed, map size, tick, every territory (owner, troops) and player
(gold, alive). Load refuses unknown versions, checks every id and troop count, regenerates the map
from the seed and compares territory count.

## 12. Audio

One looping music track and five effects (click, attack, capture, victory, defeat), all with
licenses recorded in `docs/CREDITS.md`. Master / music / effects volume in Settings.

## 13. Multiplayer (later)

Server-authoritative lockstep on the same 10 Hz tick; clients send orders, server broadcasts the
ordered order list per tick. Design only: `docs/NETWORKING.md`.

## 14. Testing strategy

- Unit + integration tests on the plain-Rust sim (`cargo test`), no window needed.
- Required: neighbour symmetry, seed determinism, combat win/lose/tie/capture, growth cap, gold
  rate, 500-tick ownership invariant, invalid orders, AI never cheats (Stage 11).
- `cargo clippy --all-targets -- -D warnings` must pass.
- Manual play-test checklist for the built `.exe` in `docs/TESTING.md` (Stage 16).

## 15. Packaging and release

`cargo build --release`, copy the exe + `assets/` + a short README into
`dist/openfront-clone-v<version>/`, test in a clean folder, `Compress-Archive`. See `docs/RELEASE.md`.

## 16. Milestones with acceptance criteria

| # | Milestone | Accepted when |
|---|---|---|
| M0 | Stages 0–3 skeleton | builds, docs folder present |
| M1 | Stages 4–5 docs | all docs exist, no TODO in core sections |
| M2 | Stage 6 sim | all required tests pass |
| M3 | Stage 7 bench | measured table in PERFORMANCE.md |
| M4 | Stages 8–10 playable | attack from the mouse, HUD matches sim |
| M5 | Stage 11 AI | AI-only match reaches a winner |
| M6 | Stage 12 menus | match → menu → new match without restart |
| M7 | Stages 13–15 | save/load (optional), polish, budgets met |
| M8 | Stages 16–19 | QA checklist dated, zip runs, tag `v1.0.0` |

## 17. Risks and fallback plans

| Risk | Fallback |
|---|---|
| Bevy API changes between versions | exact version pin (D2); upgrade only between milestones |
| Map generation too slow on Large | spatial bucket grid for nearest-seed search |
| Combined mesh too many vertices | row-run merging; then texture-based map with a palette lookup |
| AI too strong/weak | all AI numbers in `ai.toml` |
| Smart App Control blocks the unsigned exe on player PCs | document in README/RELEASE; signing is out of scope |
| Integer overflow in troops | troops are `u32`, caps keep them far below 4 × 10⁹; additions use `saturating_add` |

## 18. Glossary

- **Territory:** a connected region of map cells; the unit of ownership.
- **Cell:** one square of the map raster.
- **Neighbour:** two territories that share at least one cell edge.
- **Neutral:** owned by no player; troops never grow.
- **Tick:** one 100 ms simulation step.
- **Order:** an action a player (human or AI) asks the sim to perform next tick.
- **Troop cap:** the level at which growth stops.
- **Milli-gold:** 1/1000 gold; the internal unit so gold stays an integer.
- **Seed:** the `u64` that decides the map and setup.
