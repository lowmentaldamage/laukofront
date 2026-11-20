use crate::core::{Owner, PlayerId, Rng, TerritoryId};
use crate::sim::{Order, World};

/// **Stand-in AI** (until Stage 11): every `interval` ticks it picks a random own territory with
/// at least `min_troops`, and sends 60% of its spare troops into a random neighbour it does
/// not own. Only uses `World::queue_order`. Does not allocate.
#[derive(Debug, Clone)]
pub struct RandomAttacker {
    pub player: PlayerId,
    pub interval: u64,
    pub min_troops: u32,
    rng: Rng,
}

impl RandomAttacker {
    pub fn new(player: PlayerId, seed: u64) -> Self {
        Self {
            player,
            interval: 10,
            min_troops: 5,
            rng: Rng::new(seed ^ ((player.0 as u64) << 56) ^ 0xA1A1),
        }
    }

    /// Call once per tick, before `World::tick`. Returns the order it queued, if any.
    pub fn think(&mut self, world: &mut World) -> Option<Order> {
        // Stagger players so they do not all think on the same tick.
        if !(world.tick_count() + self.player.0 as u64).is_multiple_of(self.interval.max(1)) {
            return None;
        }
        let me = Owner::Player(self.player);
        let n = world.map().territory_count();
        let start = self.rng.below(n as u32) as usize;
        for k in 0..n {
            let from = TerritoryId(((start + k) % n) as u16);
            let t = world.territory(from);
            if t.owner != me || t.troops < self.min_troops {
                continue;
            }
            let sendable = t.sendable();
            let nbs = world.map().neighbors(from);
            if nbs.is_empty() {
                continue;
            }
            let off = self.rng.below(nbs.len() as u32) as usize;
            let target = (0..nbs.len())
                .map(|j| TerritoryId(nbs[(off + j) % nbs.len()]))
                .find(|&to| world.territory(to).owner != me);
            if let Some(to) = target {
                let order = Order::Attack {
                    player: self.player,
                    from,
                    to,
                    troops: (sendable * 60 / 100).max(1),
                };
                return world.queue_order(order).ok().map(|_| order);
            }
        }
        None
    }
}
