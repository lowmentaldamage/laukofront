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
