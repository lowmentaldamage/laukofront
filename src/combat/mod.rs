//! Attack resolution. Pure function on one territory; the caller removes the sent troops
//! from the source territory first.

use crate::core::{Owner, PlayerId};
use crate::territory::Territory;

/// What happened to the target of an attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackOutcome {
    /// Attacker had more troops: the target changes owner and keeps `remaining` troops.
    Captured { remaining: u32 },
    /// Equal troops: the defender drops to 0 and keeps the territory.
    Tie,
    /// Attacker had fewer troops: the defender keeps `defender_left` troops.
    Repelled { defender_left: u32 },
}

/// Resolves `sent` troops of `attacker` hitting `target`. Does not check adjacency or
/// ownership; `World` validates orders before calling this.
pub fn resolve(attacker: PlayerId, sent: u32, target: &mut Territory) -> AttackOutcome {
    let defenders = target.troops;
    if sent > defenders {
        let remaining = sent - defenders;
        target.owner = Owner::Player(attacker);
        target.troops = remaining;
        AttackOutcome::Captured { remaining }
    } else if sent == defenders {
        target.troops = 0;
        AttackOutcome::Tie
    } else {
        target.troops = defenders - sent;
        AttackOutcome::Repelled {
            defender_left: target.troops,
        }
    }
}
