# Decisions

One entry per decision: date, choice, reason, rejected options.

## D1. Project location
- **Date:** 2026-11-20
- **Choice:** `D:\Game-rust-openfront` instead of the guide's `D:\school-game_openfront-rust-project`.
- **Reason:** the work session was already set up in this folder; the owner chose to keep it.
- **Rejected:** creating the guide's path (would leave two folders).

## D2. Engine: Bevy 0.20.1 (exact pin)
- **Date:** 2026-11-20
- **Choice:** `bevy = "=0.20.1"`, the latest stable release on crates.io on this date.
- **Reason:** the guide fixes Rust + Bevy; an exact pin keeps builds reproducible because Bevy breaks its API between minor versions.
- **Rejected:** a caret range (`0.20`) — silent upgrades; older Bevy versions — no reason to start behind.

## D3. Helper crates
- **Date:** 2026-11-20
- `serde = "=1.0.229"` (derive): turns `game.toml` / `ai.toml` into typed structs, and later save files.
- `toml = "=1.2.1"`: parses the TOML config files.
- **No RNG crate:** a ~30-line SplitMix64 lives in `src/core/rng.rs`. Its output is fixed by our code, so the same seed gives the same map on every platform and every dependency upgrade.
- **Rejected:** `rand` + `rand_chacha` (larger, and their value stream may change across major versions); `thiserror` (one hand-written error enum is enough); `criterion` (a `std::time` bench binary is enough for Stage 7).

## D4. Toolchain
- **Date:** 2026-11-20
- **Choice:** `rust-toolchain.toml` pins `1.99.1`, the toolchain used in Stage 1. Edition 2021, as the guide asks.

## D5. Build profiles
- **Date:** 2026-11-20
- **Choice:** dev builds compile dependencies with `opt-level = 3` and our crate with `opt-level = 1`; release uses `lto = "thin"`, `codegen-units = 1`.
- **Reason:** unoptimized Bevy is too slow to play; release is the shipped build so it gets the extra optimization.

## D6. Smart App Control on the dev machine
- **Date:** 2026-11-20
- **Choice:** the owner turned Windows Smart App Control off.
- **Reason:** it blocked unsigned build scripts and proc-macro DLLs that cargo compiles (os error 4551), so `cargo build` could not finish.
- **Consequence:** players whose PCs have Smart App Control on may have the unsigned release `.exe` blocked. Noted for `RELEASE.md` and `PROJECT_STATUS.md`.

## D7. Empty Stage 3 app
- **Date:** 2026-11-20
- **Choice:** `main.rs` runs `App::new().run()`, which runs once and exits without opening a window.
- **Reason:** the guide's Stage 3 asks for an app that compiles and "opens nothing yet". The window arrives in Stage 8.
