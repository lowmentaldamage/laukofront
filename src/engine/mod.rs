//! Ports of upstream `packages/engine`: the simulation. No Bevy types, no wall-clock time,
//! no `unwrap()` outside tests.
//!
//! Ported so far (P2): players, rule formulas, tile conquest with border tracking, the execution
//! loop with the spawn phase, spawning, and per-player economy.

pub mod executions;
pub mod game;
pub mod player;
pub mod rules;
pub mod runner;
pub mod util;

pub use game::{Execution, Game};
pub use player::{Player, PlayerId, PlayerInfo, PlayerType};
pub use runner::{new_match, run_turn, Intent, MatchSetup, HUMAN};
