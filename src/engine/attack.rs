//! Attack objects and the attack's tile queue.
//! Ports upstream `game/AttackImpl.ts` and `execution/utils/FlatBinaryHeap.ts`.

use std::collections::HashSet;

use crate::engine_lib::game_map::TileRef;

use super::player::PlayerId;

/// Index into `Game`'s attack table. Ids are never reused within a match.
pub type AttackId = u32;

/// An ongoing attack: troops in the field, its front line, retreat state.
#[derive(Debug, Clone)]
pub struct Attack {
    pub(crate) id: AttackId,
    pub(crate) attacker: PlayerId,
    /// `None` = unowned land.
    pub(crate) target: Option<PlayerId>,
    pub(crate) troops: f64,
    pub(crate) source_tile: Option<TileRef>,
    pub(crate) border: HashSet<TileRef>,
    pub(crate) active: bool,
    pub(crate) retreating: bool,
    pub(crate) retreated: bool,
}

impl Attack {
    pub(crate) fn new(
        id: AttackId,
        attacker: PlayerId,
        target: Option<PlayerId>,
        troops: f64,
        source_tile: Option<TileRef>,
    ) -> Self {
        Self {
            id,
            attacker,
            target,
            troops,
            source_tile,
            border: HashSet::new(),
            active: true,
            retreating: false,
            retreated: false,
        }
    }

    pub fn id(&self) -> AttackId {
        self.id
    }

    pub fn attacker(&self) -> PlayerId {
        self.attacker
    }

    pub fn target(&self) -> Option<PlayerId> {
        self.target
    }

    pub fn troops(&self) -> f64 {
        self.troops
    }

    /// Upstream `setTroops`: never below 0.
    pub fn set_troops(&mut self, t: f64) {
        self.troops = t.max(0.0);
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn retreating(&self) -> bool {
        self.retreating
    }

    pub fn retreated(&self) -> bool {
        self.retreated
    }

    pub fn source_tile(&self) -> Option<TileRef> {
        self.source_tile
    }

    /// Tiles on the attack front.
    pub fn border_size(&self) -> usize {
        self.border.len()
    }

    pub fn border_tiles(&self) -> &HashSet<TileRef> {
        &self.border
    }
}

/// Min-heap of tiles by priority. Priorities are stored as `f32`, like upstream's `Float32Array`,
/// so ties and ordering behave the same.
#[derive(Debug, Clone, Default)]
pub struct TileHeap {
    pri: Vec<f32>,
    tiles: Vec<TileRef>,
}

impl TileHeap {
    pub fn new() -> Self {
        Self {
            pri: Vec::with_capacity(1024),
            tiles: Vec::with_capacity(1024),
        }
    }

    pub fn clear(&mut self) {
        self.pri.clear();
        self.tiles.clear();
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn push(&mut self, tile: TileRef, priority: f64) {
        let p = priority as f32;
        self.pri.push(p);
        self.tiles.push(tile);
        let mut i = self.tiles.len() - 1;
        while i > 0 {
            let parent = (i - 1) >> 1;
            if p >= self.pri[parent] {
                break;
            }
            self.pri[i] = self.pri[parent];
            self.tiles[i] = self.tiles[parent];
            i = parent;
        }
        self.pri[i] = p;
        self.tiles[i] = tile;
    }

    pub fn pop(&mut self) -> Option<TileRef> {
        if self.tiles.is_empty() {
            return None;
        }
        let top = self.tiles[0];
        let last_p = self.pri.pop().unwrap_or(0.0);
        let last_t = self.tiles.pop().unwrap_or(top);
        let len = self.tiles.len();
        if len == 0 {
            return Some(top);
        }
        let mut i = 0;
        loop {
            let left = (i << 1) + 1;
            if left >= len {
                break;
            }
            let right = left + 1;
            let child = if right < len && self.pri[right] < self.pri[left] {
                right
            } else {
                left
            };
            if last_p <= self.pri[child] {
                break;
            }
            self.pri[i] = self.pri[child];
            self.tiles[i] = self.tiles[child];
            i = child;
        }
        self.pri[i] = last_p;
        self.tiles[i] = last_t;
        Some(top)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heap_pops_in_priority_order() {
        let mut h = TileHeap::new();
        for (t, p) in [(1, 5.0), (2, 1.0), (3, 3.0), (4, 2.0), (5, 4.0), (6, 0.5)] {
            h.push(t, p);
        }
        let out: Vec<_> = std::iter::from_fn(|| h.pop()).collect();
        assert_eq!(out, vec![6, 2, 4, 3, 5, 1]);
        assert!(h.is_empty());
    }

    #[test]
    fn priorities_are_f32() {
        let mut h = TileHeap::new();
        // Equal as f32, different as f64: the first pushed stays on top (stable for equal keys).
        h.push(1, 1.000_000_01);
        h.push(2, 1.000_000_02);
        assert_eq!(h.pop(), Some(1));
    }

    #[test]
    fn troops_never_negative() {
        let mut a = Attack::new(0, 1, None, 10.0, None);
        a.set_troops(-5.0);
        assert_eq!(a.troops(), 0.0);
    }
}
