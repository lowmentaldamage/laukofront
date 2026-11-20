# Gameplay

Every rule and number. Source of truth for numbers: `assets/config/game.toml` and
`assets/config/ai.toml`. Config key names are given in `code` after each number.

## Setup

- 1 human (player 0) + 1–5 AI players. Maximum 6 players (`sim.max_players`).
- Map sizes: Small 80, Medium 150, Large 300 territories (`map.*.territories`).
- Each player starts with exactly 1 territory holding 50 troops (`territory.start_troops`).
  The first start is random; each next start is the territory farthest from all earlier starts.
- Every other territory is neutral with a random 5–20 troops
  (`territory.neutral_troops_min`, `territory.neutral_troops_max`).
- Every player starts with 0 gold.

## Time

- The simulation runs at 10 ticks per second (`sim.tick_hz`). One tick = 100 ms.
- Game speed 1×/2×/3× (Stage 12) runs 10/20/30 ticks per real second. Rules are per tick, so
  speed never changes the outcome.
- The match ends at tick 36 000 (60 minutes at 1×) at the latest (`sim.max_match_ticks`).

## Troops

- **Growth** happens on every 10th tick (`growth.interval_ticks`), i.e. once per second.
- Each owned territory below its cap gains `max(1, troops × 5 / 100)` troops, rounded down
  (`growth.percent`, `growth.min_amount`). Examples: 0→1, 19→20, 50→52, 100→105.
- Growth never takes a territory past its cap. Troops already above the cap (after a capture or
  when neighbours are lost) are kept but do not grow.
- **Neutral territories never grow.**
- **Troop cap** = `100 + 20 × (neighbours with the same owner)` (`territory.base_cap`,
  `territory.cap_per_owned_neighbor`). A territory surrounded by 6 of your own has cap 220.
  Caps are recomputed every tick.

## Gold

- Each owned territory earns its owner 60 gold per minute (`territory.gold_per_min`), paid every
  tick as 100 milli-gold (60 × 1000 / 600). Neutral land earns nothing.
- Gold is stored in milli-gold (integer), shown rounded down.
- **Reinforce** (key `R`): spend 200 gold (`reinforce.cost_gold`) to add 25 troops
  (`reinforce.troops`) to one of your territories, clamped to its cap. Refused if you have less
  than 200 gold or the territory is already at or above its cap.

## Attacking

- An attack sends troops from a territory you own to a **neighbouring** territory you do **not**
  own (neutral or enemy).
- You can send 1 to (troops − 1); 1 troop always stays behind.
- The UI offers 25%, 50% and 100% of (troops − 1), rounded down, minimum 1.
- Orders given during tick N resolve at the start of tick N+1, in the order they were given.
- With A attacking troops and D defending troops:
  - **A > D:** captured. The territory changes owner and holds A − D troops.
  - **A = D:** tie. The defender drops to 0 troops and keeps the territory.
  - **A < D:** repelled. The defender keeps D − A troops.
- The sent troops leave the source territory immediately when the order resolves.
- If, when the order resolves, you no longer own the source, or you already own the target (an
  earlier order in the same tick changed it), the order is dropped and nothing happens. If the
  source has fewer troops than ordered, the order sends (source − 1); if that is 0 it is dropped.

## Refused orders

An order is refused immediately (with a message) when: the player or territory does not exist;
the player is eliminated; the match is over; you do not own the source; the target is not a
neighbour; you own the target; 0 troops are sent; more than (troops − 1) are sent; (reinforce)
you lack 200 gold or the territory is at its cap.

## Elimination and victory

- A player with 0 territories at the end of a tick is eliminated and cannot act again.
- **Conquest:** when exactly one player is left, that player wins. (Not checked in a 1-player
  match, used only for testing and benchmarks.)
- **Time limit:** at tick 36 000 the winner is the living player with the most territories;
  ties go to the most total troops, then the lowest player id.
- After victory the simulation stops changing.

## AI (Stage 11)

AIs use exactly the same orders as the human. Difficulty (from `ai.toml`):

| | Easy | Normal | Hard |
|---|---|---|---|
| Thinks every | 20 ticks (2 s) | 10 ticks (1 s) | 10 ticks (1 s) |
| Attacks an enemy when sent ≥ | 2.0 × defenders | 1.3 × defenders | 1.15 × defenders |
| Sends | 50% | 60% | 80% |
| Reserve kept on enemy borders | 0 | 10 | 30 |
| Prefers targets that raise its caps | no | no | yes |

Each decision, in order: capture the weakest neutral neighbour it can beat; else attack the
weakest enemy neighbour if the ratio allows; else reinforce a border territory if it has ≥ 200
gold; else hold.
