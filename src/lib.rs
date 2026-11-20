//! openfront_clone: an OpenFront-style territorial conquest game.
//!
//! Simulation modules (`core`, `config`, `map`, `territory`, `combat`, `economy`, `sim`)
//! are plain Rust with no Bevy types. Presentation modules (`render`, `input`, `ui`, `menu`)
//! and `ai` sit on top of them. See `docs/ARCHITECTURE.md`.

pub mod core;
pub mod config;
pub mod map;
pub mod territory;
pub mod combat;
pub mod economy;
pub mod sim;
pub mod ai;
pub mod render;
pub mod input;
pub mod ui;
pub mod menu;
