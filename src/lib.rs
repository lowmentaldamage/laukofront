//! laukofront: an OpenFront-style territorial conquest game.
//!
//! Simulation modules (`core`, `config`, `map`, `territory`, `combat`, `economy`, `sim`)
//! are plain Rust with no Bevy types. Presentation modules (`render`, `input`, `ui`, `menu`)
//! and `ai` sit on top of them. See `docs/ARCHITECTURE.md`.

pub mod ai;
pub mod combat;
pub mod config;
pub mod core;
pub mod economy;
pub mod engine_lib;
pub mod input;
pub mod map;
pub mod menu;
pub mod render;
pub mod sim;
pub mod territory;
pub mod ui;
