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

## Implemented (P6): interface

- **Left column:** troops (bar against the cap, growth per second), attack size (clickable bar and
  the troops it sends), gold (with income per second), land (% and tiles, progress bar to the 80%
  win), attacks (outgoing with retreat buttons, incoming in red), leaderboard (colour, land %,
  troops; your row highlighted, and added at the bottom if you are not in the top 10),
  notifications (incoming attacks, eliminations, land lost, match result).
- **Top right:** clock, pause, speed, controls card. **Bottom right:** minimap with the camera's
  view; click to jump. **Bottom:** hover info and the OpenFront attribution.
- **Tutorial on the map:** four hints next to your land (spawn, expand, attack size, attack a
  player), each finished by doing it; Esc skips; remembered in
  `%APPDATA%\laukofront\settings.toml`.
- **Your land:** gold border always; pulsing rings and a "YOU" marker for 30 s after you spawn.
- **On the map:** every player's name and troops at its territory centre, sized by territory;
  conquered tiles flash in the new owner's colour; your attack fronts shimmer.
- **End screen:** victory, defeat or elimination with time and your best land share; New match
  (next seed), Keep watching, Quit.

## Implemented (P7): boats

- Click land across the water (a neighbour's coast or free land) to send a **transport ship** with
  the current attack size. It launches from your coast nearest the landing point, sails one tile
  per tick, lands, takes the landing tile and attacks from there.
- At most 3 transport ships at sea (`boats.max_transport_ships`). The landing point is the target's
  coast nearest your click (within 50 tiles) that your coast can reach by water.
- "retreat" next to a boat in the attacks card turns it home; troops landing back on your own
  coast lose 25%.
- Bots use boats too: against neighbours across narrow water (up to 4 tiles, upstream
  `shoreReachableNeighbors`) and to grab free land 5 tiles across the water. Like upstream's
  tribes, they do not cross wide oceans (that is what AI nations do, P10).

## Implemented (P8): buildings and economy

| Building | Key | Cost (upstream) | Effect |
|---|---|---|---|
| City | Q | 125K, doubling per city, max 1M | +250K max troops per level |
| Port | E | 125K, doubling per port **or** factory, max 1M | must touch the ocean; trade ships |
| Factory | F | same count as ports | trains to own cities/ports within 110 tiles pay gold |
| Defense post | G | 50K, +50K each, max 250K | attacks within 30 tiles: 5x attacker losses, 3x slower |

- Build: press the key or click the bar at the bottom, then click your land (ports snap to the
  nearest coast tile within 10). Right click or Esc cancels. Buildings need 15 tiles between them.
- Construction: cities and factories 2 s, ports and defense posts 5 s (a ring fills up).
- **Upgrade:** click one of your buildings (cities, ports, factories); costs the current price.
- **Trade:** each port sends a trade ship about every 30 s to another player's port on the same
  water; on arrival both owners get `75000 / (1 + e^(-0.03 (dist - 300))) + 50 x dist` gold.
- **Factories:** every 10 s each factory pays 10K per connected city or port (less after 9 stops),
  times its level; rails are drawn to them.
- Conquered land takes its buildings: cities, ports and factories change owner, defense posts are
  destroyed.
- The gold card shows measured income per second, split into base and trade & factories.

## Implemented (P9): warships, missiles, fallout

| Thing | Key | Cost (upstream) | Effect |
|---|---|---|---|
| Missile silo | V | 1M | launches missiles; one at a time per level, 9 s reload each |
| SAM launcher | B | 1.5M, +1.5M each, max 3M | shoots down atom and hydrogen bombs within `150 - 480 / (level + 5)` tiles (70 at level 1); 9 s reload per level |
| Warship | N | 250K, +250K each, max 1M | built at your port, patrols around the water you clicked |
| Atom bomb | Z | 750K | all land within 12 tiles, about half up to 30 |
| Hydrogen bomb | X | 5M | within 80 tiles, about half up to 100 |
| MIRV | C | 25M + 15M per MIRV launched | 350 warheads (radius 12-18) over the target player's land near the target |

- Missiles fly in a straight line (10 tiles/tick; MIRV 15) from your nearest silo with a free
  slot. A SAM can shoot one down when it is in range and the missile is within 150 tiles of its
  launch or target. SAMs cannot stop MIRVs or warheads.
- **Detonation:** tiles lose their owner and get **fallout** (dark olive). Owners lose
  `5 x troops / tiles` troops per destroyed tile (warheads use upstream's MIRV formula), and so
  do their attacks and boats. Every unit in the blast (buildings, ships) is destroyed.
- **Fallout** makes attacks into it slower and bloodier (`x (5 - 2 x fallout share)`); conquering
  a tile clears it.
- **Warships** chase enemy ships within 130 tiles: transport ships are sunk by one shot,
  warships take 200-275 damage per shot (1000 health, a shot every 2 s within 40 tiles), trade
  ships within 5 tiles are captured. Click your warship, then water, to move it.
- You get a warning when a nuke is launched at you, and a notice when a SAM shoots one down.

## Implemented (P10): diplomacy and AI nations

**Alliances** (right click a player's land for the diplomacy menu):

- Ask for an alliance; the other side has 20 s to accept (or asks back, which accepts). After a
  refusal you can ask again 30 s after your first request.
- Allies cannot attack each other; attacks on a new ally pull back, boats landing on an ally's
  coast bring the troops home, SAMs and warships leave allies alone, and missiles in flight
  between new allies are called off.
- Alliances last **5 minutes**. In the last 30 s a **renew** button shows (top-right panel); the
  alliance renews when both sides agree, otherwise it ends quietly.
- **Breaking** an alliance makes you a **traitor for 30 s**: you defend at half strength and
  attackers move faster through your land; your neighbours trust you less. Launching an atom or
  hydrogen bomb that hits 100+ tiles of an ally breaks the alliance too; a MIRV at an ally always
  does.

**Other actions** in the menu: **embargo** (no trade ships between you), **send troops / gold**
(a third of yours, allies only, every 10 s; troops never push them past their cap), **mark as
target** (your allies see it; once per 15 s, lasts 10 s) and **reactions** (short words shown on
your land for 5 s).

**Relations:** every player has a feeling toward every other one (hostile, distrustful,
neutral, friendly). Attacks, betrayals and nukes make it worse; alliances and gifts make it
better; it fades back toward neutral over time. Nations decide with it.

**AI nations** (`--nations N`, default 3 / 6 / 10 by map size) with a **difficulty**
(`--difficulty easy|medium|hard|impossible`, default medium):

| | Easy | Medium | Hard | Impossible |
|---|---|---|---|---|
| Start troops | 12.5K | 18.75K | 25K | 31.25K |
| Troop cap | x0.5 | x0.75 | x1 | x1.25 |
| Troop growth | x0.9 | x0.95 | x1 | x1.05 |
| Acts every | 6.5-10 s | 5.5-7 s | 4.5-6 s | 3-5 s |

Nations expand, answer attacks, pick targets in upstream's order for their difficulty (bots,
allies' targets, betraying weak allies, traitors, hated players, very weak or embattled enemies,
the weakest neighbour), sail to other islands, ask for and answer alliances (traitors are nearly
always refused, much stronger players are courted), build cities, ports, factories, silos, SAMs
and defense posts, launch warships, embargo players they hate, help allies under attack, and from
medium up nuke players they hate (MIRVs at a player holding over 40% of the land on hard and
impossible). Bots attack a neighbouring traitor first.

## Coming next

- **P11:** better generated maps and real-Earth maps.
