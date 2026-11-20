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
