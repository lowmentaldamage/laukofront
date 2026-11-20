# Controls

Implemented in P5–P6 (`src/input/mod.rs`, `src/ui/`, `src/render/camera.rs`). Press `H` or the
`?` button in-game for this list.

## Mouse

| Action | Effect |
|---|---|
| Left click on unclaimed land (spawn phase) | spawn there (click again to move, until the phase ends) |
| Left click on a neighbour's land or free land next to you | attack it with the current attack size |
| Left click on land across the water | send a transport ship (attack size troops; max 3 boats) |
| Left click on one of your buildings | upgrade it (city, port, factory) |
| Build bar (bottom) / Q E F G, then click your land | build city / port / factory / defense post |
| Right click (while building) | cancel |
| Left click your warship, then water | move the warship |
| Missile chosen (Z / X / C), then click | launch at that tile (blast radius shown under the cursor, enemy SAM ranges in blue) |
| Left click elsewhere | a notice says why nothing happened |
| Attack size bar (left panel) | click 10 … 90 to set the attack size |
| "retreat" next to an attack or boat (left panel) | pull it back |
| II / x1 / x2 / x3 / ? (top right) | pause, game speed, show controls |
| Minimap (bottom right) | click or drag to move the camera there |
| Right drag | pan the camera |
| Mouse wheel | zoom toward the cursor (0.5×–24×) |
| Hover | bottom panel shows the tile's terrain and owner |

Clicks on HUD panels never reach the map.

## Keyboard

| Key | Effect |
|---|---|
| `W` `A` `S` `D` / arrow keys | pan the camera |
| `1` … `9` | attack size 10% … 90% (default 20%) |
| `R` | retreat your newest attack (2 s; 25% lost if attacking a player) |
| `Space` | pause / resume |
| `F1` / `F2` / `F3` | game speed 1× / 2× / 3× |
| `Q` `E` `F` `G` `V` `B` | choose city / port / factory / defense post / missile silo / SAM |
| `N` | warship (click water near your port) |
| `Z` `X` `C` | atom bomb / hydrogen bomb / MIRV |
| `H` | show / hide the controls card |
| `Esc` | cancel building, missile or warship selection; skip the tutorial |

## Command line (until the menu exists, P12)

```powershell
laukofront --size small|medium|large --bots 20 --seed 123
laukofront --autoplay            # the game plays for you (demo)
laukofront --perf-run 60         # 1080p frame-time measurement, then exit
laukofront --screenshot shot.png --screenshot-at 10
```
