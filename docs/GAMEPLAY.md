# Gameplay

Rules ported from OpenFront (see `NOTICE.md`). Numbers come from `assets/config/game.toml`;
the key is given after each number. This file grows with each port milestone.

## Implemented (P2)

### Match setup
- One human (player 1) and any number of bots ("tribes" upstream) on a generated map
  (Small 800×500, Medium 1200×750, Large 1600×1000 tiles).
- Start troops: human 25 000 (`troops.human_start`), bot 10 000 (`troops.bot_start`).
- Start gold: 0 (`gold.starting_gold`; bots always 0).

### Spawn phase
- Lasts until the human picks a spawn, or at most 100 ticks (`game.spawn_phase_ticks`); the timer
  ends it on tick 101, as upstream.
- Bots spawn at random valid spots, at least 30 tiles (manhattan) from other spawns
  (`game.min_spawn_distance`); after 750 failed tries the distance rule is dropped; after 1 000 the
  bot does not spawn.
- A random spot must be unowned land that is not next to someone's border, with the full spawn disk
  free.
- A spawn claims every tile within radius 4 (`game.spawn_radius`) of the corner between four tiles:
  52 tiles on open land. A clicked spawn takes only the valid tiles of that disk (none on open
  water = no spawn).
- Picking again during the spawn phase moves the player. Spawn clicks sent after the spawn phase
  are ignored.
- Nothing grows during the spawn phase.

### Economy (every tick after the spawn phase)
- Troop cap = `2 × (tiles^0.6 × 1000 + 50 000)`; bots divide by 3.
- Troop growth = `(10 + troops^0.73 / 4) × (1 − troops / cap)`, bots ×0.5, never past the cap;
  above the cap troops drain toward it. The amount is floored when added (troops are integers).
- Gold: +100 per tick for humans, +50 for bots.

## Implemented (P3)

### Attacks
- An attack sends troops from your pool at one neighbour (a player or unowned land). Default size:
  20% of your troops (bots 5%) (`attack.*_default_percent`); the client can send any amount.
- The attack's front is every target tile next to your land. Each tick it conquers tiles from a
  priority queue (tiles already touched on more sides by you, and flatter terrain, go first, with a
  little randomness) until the tick's budget is spent.
- Per tile, upstream `attackLogic` (numbers in `[attack_logic]`):
  - Terrain base (mag, cost): plains 80 / 16.5, highland 100 / 20, mountain 120 / 25.
  - Unowned land: the attack loses `mag / 5` (bots `/ 10`) troops per tile; the tile costs
    `clamp(2000 × cost / attack troops, 5, 100) / (2 × front size)` of the tick.
  - A player: the attack loses
    `mag × clamp(defender troops / attack troops, 0.6, 2) × (0.463 × size bonuses + 0.0039 × defender troops per tile)`;
    the defender loses its average troops per tile; the tile costs
    `clamp(ratio, 0.82, 7.5) × clamp(ratio / 20, 1, 50) / 8.55 × cost × size bonuses / front size`.
    Size bonuses shrink losses and speed up attacks for very large territories (sigmoid around
    300 000 tiles). Humans and nations attacking bots lose 30% fewer troops.
- An attack ends when its troops drop below 1 or it runs out of tiles (survivors return home).
- Opposite attacks between the same two players cancel out; a new attack on the same target merges
  into the existing one.
- An attack with nothing to conquer next to you is refunded at once.
- **Spawn immunity:** for 50 ticks after the spawn phase (`game.spawn_immunity_ticks`), humans
  cannot attack humans or nations. Bots ignore immunity and are never immune.

### Retreat
- Cancelling an attack freezes it at once; 20 ticks later (`attack_logic.retreat_delay_ticks`) the
  troops come home, minus 25% when retreating from a player (`retreat_malus_percent`).

### Defeat
- A defender under 100 tiles that is attacked is finished off: tiles touching the attacker go to
  the attacker, others to another enemy neighbour. The attacker takes its gold (all of a bot's or
  nation's, half of a human's, nothing from a human who never attacked).
- **Encirclement:** a piece of land cut off from the coast, the map edge and unowned land, and
  surrounded by enemies, is annexed by the enemy with the largest attack on its owner (else the one
  with the longest shared border). A player's largest piece is only annexed if a single enemy
  surrounds it. Checked every 20 ticks per player after its land changed (`cluster_check_ticks`).

### Winning
- Every 10 ticks: the player with the most tiles wins once they own more than 80% of the land
  (`game.win_percent`), or at the hard time limit of 170 minutes.

## Implemented (P4): bots

Port of upstream tribe AI (`TribeExecution`, `TribeAttackBehavior`, land parts of `AiAttackBehavior`).
- Each bot draws its own personality from its id: acts every 40–80 ticks; attacks once its troops
  reach 50–60% of its cap (trigger); keeps 30–40% of its cap at home when attacking a player
  (reserve) and 10–20% when expanding (expand).
- First action: expand into free land.
- Then, while it still borders free land, it keeps expanding. Otherwise, once it has the trigger
  ratio: strike back at whoever has the largest attack on it, else attack a random land neighbour
  (humans and nations are skipped half the time).
- An attack sends everything above the reserve. Bots only use the same attacks a human can.
- Not yet: boat attacks and river crossings (transport ships), alliances and traitors.

## Implemented (P5): playing it

- `cargo run --release` opens a match (Medium map, 20 bots, random seed; see `CONTROLS.md`).
- Spawn phase: click unclaimed land within 10 s, or you watch the bots.
- Click land next to your border to attack it with the chosen share of your troops (keys 1-9).
- HUD: time, troops / cap, gold, share of land, attack size, active attacks; leaderboard of the top
  10 by land; hover info; notices; help line and the OpenFront attribution.

## Coming next

- **Boats (P6):** transport ships, so players on separate landmasses can reach each other.
