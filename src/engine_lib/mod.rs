//! Ports of upstream `packages/engine-lib`: the deterministic building blocks the engine and the
//! client share. No Bevy types, no wall-clock time, no platform transcendental maths.

pub mod detmath;
pub mod game_map;
pub mod random;
pub mod terrain;
