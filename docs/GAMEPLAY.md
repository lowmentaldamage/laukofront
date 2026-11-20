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

## Coming next

- **P3 attacks:** troops sent into neighbouring land conquer it tile by tile using upstream
  `attackLogic` (terrain, troop ratio, defender density, large-territory bonuses); retreat; a player
  owning 80% of land (`game.win_percent`) wins.
- **P4 bots:** upstream tribe AI.
