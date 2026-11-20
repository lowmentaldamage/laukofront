use std::fmt;

/// Why an order was refused by `World::queue_order`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderError {
    UnknownPlayer,
    UnknownTerritory,
    PlayerEliminated,
    GameOver,
    NotOwner,
    NotNeighbor,
    OwnTarget,
    NoTroopsSent,
    NotEnoughTroops,
    NotEnoughGold,
    AtCap,
}

impl fmt::Display for OrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            OrderError::UnknownPlayer => "unknown player",
            OrderError::UnknownTerritory => "unknown territory",
            OrderError::PlayerEliminated => "player is eliminated",
            OrderError::GameOver => "the match is over",
            OrderError::NotOwner => "you do not own that territory",
            OrderError::NotNeighbor => "target is not a neighbour",
            OrderError::OwnTarget => "you already own the target",
            OrderError::NoTroopsSent => "must send at least 1 troop",
            OrderError::NotEnoughTroops => "not enough troops (1 must stay behind)",
            OrderError::NotEnoughGold => "not enough gold",
            OrderError::AtCap => "territory is already at its troop cap",
        };
        f.write_str(msg)
    }
}

/// The project-wide error type.
#[derive(Debug)]
pub enum GameError {
    /// A config file could not be read.
    Io {
        path: String,
        source: std::io::Error,
    },
    /// A config file is malformed, has a missing/unknown key, or a value out of range.
    Config { source: String, message: String },
    /// Requested setup cannot be built (e.g. more players than territories).
    Setup(String),
    /// An order was refused.
    Order(OrderError),
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::Io { path, source } => write!(f, "cannot read {path}: {source}"),
            GameError::Config { source, message } => write!(f, "bad config in {source}: {message}"),
            GameError::Setup(msg) => write!(f, "cannot set up match: {msg}"),
            GameError::Order(e) => write!(f, "order refused: {e}"),
        }
    }
}

impl std::error::Error for GameError {}

impl From<OrderError> for GameError {
    fn from(e: OrderError) -> Self {
        GameError::Order(e)
    }
}
