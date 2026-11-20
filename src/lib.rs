//! laukofront: a Rust port of the OpenFront territorial strategy engine (see NOTICE.md).
//!
//! - `engine_lib`, `engine`, `config`: the simulation, plain Rust, no Bevy types.
//! - `render`, `input`, `ui`, `menu`: the Bevy client (milestone P5).

pub mod client;
pub mod config;
pub mod engine;
pub mod engine_lib;
pub mod input;
pub mod menu;
pub mod render;
pub mod ui;
