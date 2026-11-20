# Controls

Implemented in P5–P6 (`src/input/mod.rs`, `src/ui/`, `src/render/camera.rs`). Press `H` or the
`?` button in-game for this list.

## Mouse

| Action | Effect |
|---|---|
| Left click on unclaimed land (spawn phase) | spawn there (click again to move, until the phase ends) |
| Left click on a neighbour's land or free land next to you | attack it with the current attack size |
| Left click elsewhere | a notice says why nothing happened |
| Attack size bar (left panel) | click 10 … 90 to set the attack size |
| "retreat" next to an attack (left panel) | pull that attack back |
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
| `H` | show / hide the controls card |
| `Esc` | skip the tutorial |

## Command line (until the menu exists, P12)

```powershell
laukofront --size small|medium|large --bots 20 --seed 123
laukofront --autoplay            # the game plays for you (demo)
laukofront --perf-run 60         # 1080p frame-time measurement, then exit
laukofront --screenshot shot.png --screenshot-at 10
```
