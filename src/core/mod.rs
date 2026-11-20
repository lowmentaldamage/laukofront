//! Shared building blocks: ids, ownership, the error type and the deterministic RNG.
//! No Bevy types and no dependencies on other game modules.

mod error;
mod rng;

pub use error::{GameError, OrderError};
pub use rng::Rng;

/// Index of a territory on the map. Valid ids are `0..map.territory_count()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerritoryId(pub u16);

impl TerritoryId {
    /// The id as a slice index.
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Index of a player. Valid ids are `0..world.players().len()`; player 0 is the human.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u8);

impl PlayerId {
    /// The id as a slice index.
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Who owns a territory. Every territory has exactly one `Owner` at all times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    Neutral,
    Player(PlayerId),
}

impl Owner {
    /// The owning player, or `None` for neutral land.
    #[inline]
    pub fn player(self) -> Option<PlayerId> {
        match self {
            Owner::Neutral => None,
            Owner::Player(p) => Some(p),
        }
    }
}

/// Hard limit from the id type: territory ids are `u16`.
pub const MAX_TERRITORIES: usize = u16::MAX as usize;
/// Hard limit from the id type: player ids are `u8`.
pub const MAX_PLAYERS: usize = u8::MAX as usize;
/// Milli-gold per gold.
pub const MILLI: u64 = 1000;
