//! Per-territory and per-player state. Plain data; rules live in `combat` and `economy`.

use crate::core::{Owner, PlayerId};

/// State of one territory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Territory {
    pub owner: Owner,
    pub troops: u32,
    /// Growth stops at this value. Recomputed every tick by `economy::update_caps`.
    pub troop_cap: u32,
    /// Gold this territory earns its owner per minute. Neutral land earns nothing.
    pub gold_per_min: u32,
}

impl Territory {
    /// Troops that can leave in one attack: everything except the 1 that must stay.
    #[inline]
    pub fn sendable(&self) -> u32 {
        self.troops.saturating_sub(1)
    }
}

/// State of one player. `territories` and `troops` are totals refreshed every tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerState {
    pub id: PlayerId,
    /// Gold in milli-gold (1 gold = 1000).
    pub gold_milli: u64,
    pub alive: bool,
    pub territories: u32,
    pub troops: u64,
}

impl PlayerState {
    /// Whole gold, rounded down.
    #[inline]
    pub fn gold(&self) -> u64 {
        self.gold_milli / crate::core::MILLI
    }
}
