# Controls

Implemented in P5 (`src/input/mod.rs`, `src/render/camera.rs`). Shown in-game in the bottom-right
help line.

## Mouse

| Action | Effect |
|---|---|
| Left click on unclaimed land (spawn phase) | spawn there (click again to move, until the phase ends) |
| Left click on a neighbour's land or free land next to you | attack it with the current attack size |
| Left click elsewhere | a notice says why nothing happened (your land, not a neighbour, water, spawn immunity, not enough troops) |
| Right drag | pan the camera |
| Mouse wheel | zoom toward the cursor (0.5×–24× of "whole map fits") |
| Hover | bottom-left panel shows the tile's terrain and owner |

## Keyboard

| Key | Effect |
|---|---|
| `W` `A` `S` `D` / arrow keys | pan the camera |
| `1` … `9` | attack size 10% … 90% of your troops (default 20%) |
| `R` | retreat your newest attack (troops return after 2 s; 25% lost if attacking a player) |
| `Space` | pause / resume |
| `F1` / `F2` / `F3` | game speed 1× / 2× / 3× |

## Command line (until the menu exists, Stage 12)

```powershell
laukofront --size small|medium|large --bots 20 --seed 123
laukofront --autoplay            # the game plays for you (demo)
laukofront --perf-run 60         # 1080p frame-time measurement, then exit
laukofront --screenshot shot.png --screenshot-at 10
```
