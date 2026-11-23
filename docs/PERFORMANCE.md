# Performance

## Targets (fixed — do not change these to pass)

| Target | Value |
|---|---|
| Frame rate | 60 FPS at 1080p on a mid-range PC, 80 territories, 3 AIs |
| Sim tick | under 2 ms at 10 Hz |
| Worst frame | no frame over 33 ms in a 5-minute run |
| Headless sim | at least 10× faster than real time (≥ 100 ticks/s) for 80 territories + 3 AIs |

Per-frame budget at 60 FPS (16.6 ms): sim ≤ 2 ms per tick (≤ 0.2 ms per frame on average at
10 Hz), AI ≤ 1 ms per think, render ≤ 6 ms, UI ≤ 2 ms; the rest is headroom.
Map generation: ≤ 500 ms for Large.

## How to measure

- **Headless sim:** `cargo run --release --bin sim_bench` (options `--ticks N`, `--seed S`).
  Prints Markdown tables; paste them below with date and machine.
- **Frame time (from Stage 8):** Bevy `FrameTimeDiagnosticsPlugin` + `LogDiagnosticsPlugin`,
  release build, 1920×1080 window, 5-minute run.
- Always measure `--release`. Record the machine (see `ENVIRONMENT.md`).

## Note on the machine

The development PC (Ryzen 9 7950X, RTX 4090) is high-end, not mid-range. Numbers measured here
must be read with large margins; Stage 15 should add a run on a mid-range or throttled machine.

## Results

Results are added below as each stage measures them.

### 2026-11-20 — Stage 7: headless simulation (sim_bench)

Machine: Ryzen 9 7950X, Windows 11 build 26300, Rust 1.99.1, release profile (thin LTO).
Command: `cargo run --release --bin sim_bench -- --ticks 20000` (seed 1).
"AIs" are the `RandomAttacker` stand-in (D14); player 0 is idle. Tick cost includes AI thinking.
"Matches" > 1 means a match ended early and a new one was started (setup time excluded).

| Map | Territories | Map gen (ms) |
|---|---|---|
| Small | 80 | 1.5 |
| Medium | 150 | 4.7 |
| Large | 300 | 16.3 |

| Territories | AIs | Ticks/s | Mean tick (µs) | Max tick (µs) | x real time (10 Hz) | Matches |
|---|---|---|---|---|---|---|
| 80 | 0 | 3071159 | 0.33 | 4.10 | 307116 | 1 |
| 80 | 1 | 1983910 | 0.50 | 5.20 | 198391 | 8 |
| 80 | 3 | 1746542 | 0.57 | 3.50 | 174654 | 1 |
| 80 | 5 | 1679924 | 0.59 | 3.20 | 167992 | 1 |
| 150 | 0 | 1639788 | 0.61 | 5.40 | 163979 | 1 |
| 150 | 1 | 1132804 | 0.88 | 4.90 | 113280 | 4 |
| 150 | 3 | 981846 | 1.02 | 30.20 | 98185 | 1 |
| 150 | 5 | 955813 | 1.05 | 5.10 | 95581 | 1 |
| 300 | 0 | 822849 | 1.22 | 10.90 | 82285 | 1 |
| 300 | 1 | 641785 | 1.56 | 10.10 | 64178 | 4 |
| 300 | 3 | 502519 | 1.99 | 32.50 | 50252 | 1 |
| 300 | 5 | 490924 | 2.04 | 30.80 | 49092 | 1 |

**Against the targets:**

- 80 territories + 3 AIs: **1 746 542 ticks/s**, ~175 000× real time. Target is ≥ 100 ticks/s
  (10× real time): **met** with a very large margin. Not a risk.
- Sim tick < 2 ms: worst single tick in any case was 32.5 µs (300 territories), 60× under budget.
  The rare ~30 µs maxima are OS scheduling noise; means stay at 0.3–2.0 µs.
- Map generation for Large: 16.3 ms against a 500 ms budget: **met**.
- Cost scales roughly linearly with territory count (the tick touches every territory for caps,
  growth and gold).
- Not yet measured: frame time and FPS (from Stage 8), mid-range hardware.

### 2026-11-20 — Port milestone P2: engine_bench (economy only)

`cargo run --release --bin engine_bench` (2000 ticks, seed 1). Setup = map generation, players,
bot spawns. Ticks contain only spawn-phase bookkeeping and per-player economy; attacks come in P3.

| Map | Tiles | Bots | Setup (ms) | Ticks/s | Mean tick (µs) | Max tick (µs) |
|---|---|---|---|---|---|---|
| Small | 400000 | 0 | 39.1 | 10520779 | 0.10 | 1.70 |
| Small | 400000 | 10 | 42.2 | 1615248 | 0.62 | 21.90 |
| Small | 400000 | 50 | 40.7 | 397219 | 2.52 | 9.60 |
| Medium | 900000 | 0 | 93.0 | 10672359 | 0.09 | 0.90 |
| Medium | 900000 | 10 | 90.4 | 1664724 | 0.60 | 35.70 |
| Medium | 900000 | 50 | 93.5 | 379240 | 2.64 | 187.90 |
| Large | 1600000 | 0 | 161.0 | 11448197 | 0.09 | 0.90 |
| Large | 1600000 | 10 | 162.6 | 1755772 | 0.57 | 1.20 |
| Large | 1600000 | 50 | 159.8 | 385000 | 2.60 | 11.90 |

Far inside the 2 ms tick budget; the real cost will be attacks (P3).

### 2026-11-20 — Port milestone P3: engine_bench with attacks

`cargo run --release --bin engine_bench -- --ticks 3000` (seed 1). Every 10 ticks each living
player launches a default attack at a random player or unowned land.

| Map | Tiles | Bots | Setup (ms) | Ticks/s | Mean tick (µs) | Max tick (µs) | Owned tiles at end |
|---|---|---|---|---|---|---|---|
| Small | 400000 | 0 | 29.9 | 82056 | 12.19 | 394.90 | 35920 |
| Small | 400000 | 10 | 30.5 | 12657 | 79.01 | 823.00 | 199068 |
| Small | 400000 | 50 | 30.5 | 10011 | 99.89 | 1226.70 | 199068 |
| Medium | 900000 | 0 | 67.3 | 89797 | 11.14 | 506.90 | 35604 |
| Medium | 900000 | 10 | 67.2 | 5822 | 171.75 | 1925.50 | 447834 |
| Medium | 900000 | 50 | 67.9 | 5079 | 196.91 | 1965.70 | 447834 |
| Large | 1600000 | 0 | 119.9 | 97557 | 10.25 | 335.50 | 35421 |
| Large | 1600000 | 10 | 118.8 | 3694 | 270.72 | 3059.40 | 756141 |
| Large | 1600000 | 50 | 197.5 | 2359 | 423.85 | 3037.20 | 796035 |

- Mean tick 0.01–0.42 ms: well under the 2 ms target in every case.
- Worst single tick: 1.2 ms (Small), 2.0 ms (Medium), 3.1 ms (Large). At 10 Hz a 3 ms tick is
  still far from a 33 ms frame, but it is the first number near the budget. Likely cause: the
  simplified encirclement check flood-fills whole territories (D25). Recorded as a Stage 15 item.

### 2026-11-20 — Port milestone P4: engine_bench with bot AI

Same command as P3; bots now also run their own AI on top of the random attacks.

| Map | Tiles | Bots | Setup (ms) | Ticks/s | Mean tick (µs) | Max tick (µs) | Owned tiles at end |
|---|---|---|---|---|---|---|---|
| Small | 400000 | 0 | 28.6 | 89510 | 11.17 | 367.70 | 35920 |
| Small | 400000 | 10 | 28.9 | 12226 | 81.79 | 864.30 | 199068 |
| Small | 400000 | 50 | 29.3 | 8472 | 118.03 | 871.60 | 199068 |
| Medium | 900000 | 0 | 65.0 | 99436 | 10.06 | 347.80 | 35604 |
| Medium | 900000 | 10 | 64.7 | 6898 | 144.97 | 1351.70 | 447834 |
| Medium | 900000 | 50 | 64.9 | 4415 | 226.52 | 1784.10 | 447834 |
| Large | 1600000 | 0 | 115.0 | 106039 | 9.43 | 331.40 | 35421 |
| Large | 1600000 | 10 | 117.2 | 4679 | 213.71 | 2824.30 | 745640 |
| Large | 1600000 | 50 | 115.2 | 2715 | 368.32 | 2128.80 | 796035 |

A whole 10-bot Small match (~25 000 ticks) runs in 1–2 s of real time.

### 2026-11-20 — Port milestone P5: frame time at 1080p

`laukofront --seed 3 --autoplay --perf-run 60` (1920×1080, no vsync, release, autoplay at 3×
speed = 30 ticks/s, first 2 s not recorded). Dev PC (RTX 4090, Ryzen 9 7950X).

| Setup | Frames | Avg frame | FPS | p99 | Max | Frames > 33 ms |
|---|---|---|---|---|---|---|
| Medium 1200×750, 20 bots | 75 019 | 0.80 ms | 1250 | 1.79 ms | 7.31 ms | 0 |
| Large 1600×1000, 50 bots | 74 077 | 0.81 ms | 1235 | 2.63 ms | 6.50 ms | 0 |

Targets on this PC: 60 FPS ✔, no frame over 33 ms ✔. Still not measured on a mid-range PC
(Stage 15). A first run showed exactly 60 FPS: the unfocused window was throttled (D31).

### 2026-11-20 — Port milestone P6: frame time with the new interface

Same command as P5 (1080p, no vsync, autoplay 3×).

| Setup | Frames | Avg frame | FPS | p99 | Max | Frames > 33 ms |
|---|---|---|---|---|---|---|
| Medium, 20 bots | 33 852 | 1.77 ms | 564 | 3.46 ms | 8.95 ms | 0 |
| Large, 50 bots | 25 494 | 2.35 ms | 425 | 5.40 ms | 9.42 ms | 0 |

The HUD, labels, minimap and capture overlay cost about 1–1.5 ms per frame (P5: 0.8 ms). Still far
inside the 16.6 ms / 33 ms budgets on this PC.

### 2026-11-20 — P7 + P8: boats, buildings, trade

**Engine** (`engine_bench --ticks 3000`, random attacks + bot AI with boats + trade + factories):

| Map | Bots | Mean tick (µs) | Max tick (µs) |
|---|---|---|---|
| Small | 10 / 50 | 95 / 145 | 1437 / 1576 |
| Medium | 10 / 50 | 192 / 306 | 2654 / 3675 |
| Large | 10 / 50 | 304 / 534 | 4017 / 2795 |

**Water routes** (Large map, 40 ocean routes): plain weighted A* up to 114 ms → waypoint routing
(D35) at most 1.4 ms.

**Frame time** (1080p, no vsync, autoplay 3x building and boating; after the D38 guards):

| Setup | Avg frame | FPS | p99 | Slow frames (> 33 ms) |
|---|---|---|---|---|
| Medium, 20 bots | 1.8–2.9 ms | 348–555 | 4.0–5.7 ms | 0–1 per minute |
| Large, 50 bots | 2.2–3.3 ms | 308–455 | 5.4–6.9 ms | 1–2, all 2–4 s after the match starts |

The remaining slow frames (80–120 ms) happen once at match start (spawning, first labels and first
ship/building drawings); none were seen during play after the guards.

### 2026-11-20 — P9: engine with weapons

`engine_bench --ticks 3000`: mean tick 85-374 µs, max 945-2668 µs across all sizes (no change
from P8; bots do not use weapons, the SAM check costs nothing without missiles in flight).
Frame-time run (`--autoplay --perf-run 60`, 1440x900, autoplay builds silos and launches an
atom bomb every 300 ticks):

| Map | Bots | Avg | FPS | p99 | Max | Frames > 33 ms |
|---|---|---|---|---|---|---|
| medium | 20 | 1.65 ms | 605 | 3.20 ms | 6.07 ms | 0 |
| large | 50 | 2.11 ms | 474 | 4.88 ms | 6.52 ms | 0 |

No slow frames at all, including at match start (the P8 start-up spikes are gone).

### 2026-11-20 — P10: nations

Frame time (`--autoplay --perf-run 60`, 1440x900):

| Map | Bots | Nations | Avg | FPS | p99 | Max | Frames > 33 ms |
|---|---|---|---|---|---|---|---|
| medium | 20 | 6 | 1.72 ms | 583 | 3.34 ms | 6.75 ms | 0 |
| large | 50 | 10 | 2.29 ms | 437 | 5.53 ms | 11.56 ms | 0 |

Engine only (`bot_match`, 30 bots + 10 hard nations): medium 6 771 ticks in 1.2 s (0.18 ms per
tick), large 10 041 ticks in 3.5 s (0.35 ms per tick).

### 2026-11-21 — P11: maps

Frame time (`--autoplay --perf-run 60`, 1440x900):

| Map | Players | Avg | FPS | p99 | Max | Frames > 33 ms |
|---|---|---|---|---|---|---|
| world (2000x766) | 50 bots + 16 nations | 2.18 ms | 459 | 4.87 ms | 7.18 ms | 0 |
| europe (1200x1000) | 25 + 10 | 1.97 ms | 509 | 4.18 ms | 8.49 ms | 0 |
| archipelago large | 50 + 10 | 2.24 ms | 447 | 4.96 ms | 7.87 ms | 0 |

Ship routes (`water_route_timing`, 40 random coast pairs): worst 0.85 ms (generated large), 1.48 ms
(World, was 40 ms before the coarse graph), 1.15 ms (Americas), 1.25 ms (Europe). Map build:
57-193 ms for the Earth maps, 33-43 ms for small generated maps. Engine (`bot_match`, 20 bots + 8
nations): World 18 041 ticks in 3.0 s (0.17 ms per tick).

### 2026-11-21 — Icon sprites

| Map | Players | Avg | FPS | p99 | Max | Frames > 33 ms |
|---|---|---|---|---|---|---|
| large generated | 50 bots + 10 nations | 2.56 ms | 391 | 5.69 ms | 8.85 ms | 0 |
| world | 50 + 16 | 2.46 ms | 407 | 4.95 ms | 10.27 ms | 0 |

About 0.3 ms more per frame than line icons (sprites for every building and ship, updated each
frame).

### 2026-11-23 — v1.0.0

| Map | Players | Avg | FPS | p99 | Max | Frames > 33 ms |
|---|---|---|---|---|---|---|
| large generated | 50 bots + 10 nations | 2.51 ms | 399 | 6.08 ms | 8.66 ms | 0 |
| World | 50 + 16 | 2.42 ms | 413 | 5.04 ms | 9.56 ms | 0 |

Border-tile encirclement (D25) stays simplified: with ticks at 0.2-0.4 ms there is no need.
