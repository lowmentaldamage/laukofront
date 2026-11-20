//! Ports of upstream `packages/engine`: the simulation. No Bevy types, no wall-clock time,
//! no `unwrap()` outside tests.
//!
//! Ported so far: players, rule formulas, tile conquest with border tracking, the execution loop
//! with the spawn phase, spawning, per-player economy (P2); attacks, retreat, defeated-player
//! takeover, encircled-territory annexation and the win check (P3); bot AI (P4).

pub mod attack;
pub mod attack_exec;
pub mod boats;
pub mod bots;
pub mod clusters;
pub mod diplomacy;
pub mod executions;
pub mod game;
pub mod nations;
pub mod player;
pub mod rules;
pub mod runner;
pub mod structures;
pub mod units;
pub mod util;
pub mod water;
pub mod weapons;

pub use game::{Execution, Game};
pub use player::{Player, PlayerId, PlayerInfo, PlayerType};
pub use runner::{new_match, run_turn, Intent, MatchSetup, HUMAN};
