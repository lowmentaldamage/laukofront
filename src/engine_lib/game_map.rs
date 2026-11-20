//! Port of upstream `packages/engine-lib/src/game/GameMapImpl.ts`: the tile grid.
//!
//! Terrain byte per tile (immutable during a match):
//! bit 7 land, bit 6 shoreline, bit 5 ocean, bits 0-4 magnitude (31 = impassable land).
//! State word per tile (mutable): bits 0-11 owner small id (0 = nobody), bit 13 fallout,
//! bit 14 defense bonus.

/// Index of a tile: `y * width + x`.
pub type TileRef = u32;

pub const IS_LAND_BIT: u8 = 7;
pub const SHORELINE_BIT: u8 = 6;
pub const OCEAN_BIT: u8 = 5;
pub const MAGNITUDE_MASK: u8 = 0x1f;
pub const IMPASSABLE_MAGNITUDE: u8 = 31;
pub const PLAYER_ID_MASK: u16 = 0xfff;
pub const FALLOUT_BIT: u16 = 13;
pub const DEFENSE_BONUS_BIT: u16 = 14;

/// Upstream `TerrainType` for tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerrainType {
    Plains,
    Highland,
    Mountain,
    Impassable,
    Ocean,
    Lake,
}

#[derive(Debug, Clone)]
pub struct GameMap {
    width: u32,
    height: u32,
    terrain: Vec<u8>,
    state: Vec<u16>,
    num_land_tiles: u32,
}

impl GameMap {
    /// Builds a map from terrain bytes. Returns `None` if the length does not match.
    pub fn new(width: u32, height: u32, terrain: Vec<u8>) -> Option<Self> {
        if terrain.len() != width as usize * height as usize || width == 0 || height == 0 {
            return None;
        }
        let num_land_tiles = terrain.iter().filter(|&&b| is_passable_land(b)).count() as u32;
        let state = vec![0; terrain.len()];
        Some(Self {
            width,
            height,
            terrain,
            state,
            num_land_tiles,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn num_tiles(&self) -> u32 {
        self.width * self.height
    }

    /// Passable land tiles (what the 80% win rule counts).
    pub fn num_land_tiles(&self) -> u32 {
        self.num_land_tiles
    }

    /// Tile at `(x, y)`, or `None` outside the map.
    pub fn tile(&self, x: i64, y: i64) -> Option<TileRef> {
        (x >= 0 && y >= 0 && x < self.width as i64 && y < self.height as i64)
            .then(|| (y as u32) * self.width + x as u32)
    }

    pub fn is_valid(&self, t: TileRef) -> bool {
        t < self.num_tiles()
    }

    #[inline]
    pub fn x(&self, t: TileRef) -> u32 {
        t % self.width
    }

    #[inline]
    pub fn y(&self, t: TileRef) -> u32 {
        t / self.width
    }

    /// Raw terrain bytes (for the renderer).
    pub fn terrain_bytes(&self) -> &[u8] {
        &self.terrain
    }

    // ----- terrain -----

    #[inline]
    pub fn is_land(&self, t: TileRef) -> bool {
        self.terrain[t as usize] & (1 << IS_LAND_BIT) != 0
    }

    #[inline]
    pub fn is_water(&self, t: TileRef) -> bool {
        !self.is_land(t)
    }

    #[inline]
    pub fn is_ocean(&self, t: TileRef) -> bool {
        self.terrain[t as usize] & (1 << OCEAN_BIT) != 0
    }

    #[inline]
    pub fn is_lake(&self, t: TileRef) -> bool {
        !self.is_land(t) && !self.is_ocean(t)
    }

    #[inline]
    pub fn is_shoreline(&self, t: TileRef) -> bool {
        self.terrain[t as usize] & (1 << SHORELINE_BIT) != 0
    }

    /// Land tile next to water.
    #[inline]
    pub fn is_shore(&self, t: TileRef) -> bool {
        self.is_land(t) && self.is_shoreline(t)
    }

    /// Land tile with an ocean 4-neighbour (upstream `isOceanShore`).
    pub fn is_ocean_shore(&self, t: TileRef) -> bool {
        self.is_land(t) && self.for_each_neighbor_any(t, |n| self.is_ocean(n))
    }

    #[inline]
    pub fn magnitude(&self, t: TileRef) -> u8 {
        self.terrain[t as usize] & MAGNITUDE_MASK
    }

    #[inline]
    pub fn is_impassable(&self, t: TileRef) -> bool {
        self.is_land(t) && self.magnitude(t) == IMPASSABLE_MAGNITUDE
    }

    /// Land that can be owned and attacked.
    #[inline]
    pub fn is_passable_land(&self, t: TileRef) -> bool {
        is_passable_land(self.terrain[t as usize])
    }

    pub fn terrain_type(&self, t: TileRef) -> TerrainType {
        if self.is_land(t) {
            match self.magnitude(t) {
                m if m >= IMPASSABLE_MAGNITUDE => TerrainType::Impassable,
                m if m < 10 => TerrainType::Plains,
                m if m < 20 => TerrainType::Highland,
                _ => TerrainType::Mountain,
            }
        } else if self.is_ocean(t) {
            TerrainType::Ocean
        } else {
            TerrainType::Lake
        }
    }

    /// Upstream `cost`: 2 for low land, 1 otherwise (used by pathfinding).
    pub fn cost(&self, t: TileRef) -> u32 {
        if self.magnitude(t) < 10 {
            2
        } else {
            1
        }
    }

    // ----- ownership -----

    /// Owner small id, 0 = nobody (terra nullius).
    #[inline]
    pub fn owner_id(&self, t: TileRef) -> u16 {
        self.state[t as usize] & PLAYER_ID_MASK
    }

    #[inline]
    pub fn has_owner(&self, t: TileRef) -> bool {
        self.owner_id(t) != 0
    }

    /// Sets the owner small id. Ids above 4095 are rejected (returns false), as upstream throws.
    pub fn set_owner_id(&mut self, t: TileRef, id: u16) -> bool {
        if id > PLAYER_ID_MASK {
            return false;
        }
        let s = &mut self.state[t as usize];
        *s = (*s & !PLAYER_ID_MASK) | id;
        true
    }

    pub fn has_fallout(&self, t: TileRef) -> bool {
        self.state[t as usize] & (1 << FALLOUT_BIT) != 0
    }

    pub fn has_defense_bonus(&self, t: TileRef) -> bool {
        self.state[t as usize] & (1 << DEFENSE_BONUS_BIT) != 0
    }

    /// Touches the map edge or impassable terrain.
    pub fn is_on_edge_of_map(&self, t: TileRef) -> bool {
        let w = self.width;
        let x = t % w;
        if x == 0 || x == w - 1 || t < w || t >= (self.height - 1) * w {
            return true;
        }
        self.is_impassable(t - 1)
            || self.is_impassable(t + 1)
            || self.is_impassable(t - w)
            || self.is_impassable(t + w)
    }

    /// A 4-neighbour has a different owner.
    pub fn is_border(&self, t: TileRef) -> bool {
        let owner = self.owner_id(t);
        self.for_each_neighbor_any(t, |n| self.owner_id(n) != owner)
    }

    // ----- neighbours (order N, S, W, E as upstream) -----

    /// Writes the 4-neighbours into `out` and returns how many there are.
    #[inline]
    pub fn neighbors4(&self, t: TileRef, out: &mut [TileRef; 4]) -> usize {
        let w = self.width;
        let x = t % w;
        let mut n = 0;
        if t >= w {
            out[n] = t - w;
            n += 1;
        }
        if t < (self.height - 1) * w {
            out[n] = t + w;
            n += 1;
        }
        if x != 0 {
            out[n] = t - 1;
            n += 1;
        }
        if x != w - 1 {
            out[n] = t + 1;
            n += 1;
        }
        n
    }

    /// True if `pred` holds for any 4-neighbour.
    #[inline]
    pub fn for_each_neighbor_any(&self, t: TileRef, mut pred: impl FnMut(TileRef) -> bool) -> bool {
        let mut buf = [0; 4];
        let n = self.neighbors4(t, &mut buf);
        buf[..n].iter().any(|&nb| pred(nb))
    }

    pub fn manhattan_dist(&self, a: TileRef, b: TileRef) -> u32 {
        self.x(a).abs_diff(self.x(b)) + self.y(a).abs_diff(self.y(b))
    }

    pub fn euclidean_dist_squared(&self, a: TileRef, b: TileRef) -> u64 {
        let dx = self.x(a).abs_diff(self.x(b)) as u64;
        let dy = self.y(a).abs_diff(self.y(b)) as u64;
        dx * dx + dy * dy
    }
}

#[inline]
fn is_passable_land(b: u8) -> bool {
    b & (1 << IS_LAND_BIT) != 0 && b & MAGNITUDE_MASK != IMPASSABLE_MAGNITUDE
}

/// Builds a terrain byte.
pub fn terrain_byte(land: bool, shoreline: bool, ocean: bool, magnitude: u8) -> u8 {
    ((land as u8) << IS_LAND_BIT)
        | ((shoreline as u8) << SHORELINE_BIT)
        | ((ocean as u8) << OCEAN_BIT)
        | (magnitude & MAGNITUDE_MASK)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_3x3() -> GameMap {
        // Centre tile is a mountain, corner (0,0) is ocean, rest plains.
        let mut t = vec![terrain_byte(true, false, false, 5); 9];
        t[4] = terrain_byte(true, false, false, 25);
        t[0] = terrain_byte(false, false, true, 0);
        GameMap::new(3, 3, t).unwrap()
    }

    #[test]
    fn neighbors_follow_upstream_order() {
        let m = map_3x3();
        let mut out = [0; 4];
        let n = m.neighbors4(4, &mut out);
        assert_eq!(&out[..n], &[1, 7, 3, 5]); // N, S, W, E
        let n = m.neighbors4(0, &mut out);
        assert_eq!(&out[..n], &[3, 1]);
    }

    #[test]
    fn terrain_queries() {
        let m = map_3x3();
        assert_eq!(m.num_land_tiles(), 8);
        assert_eq!(m.terrain_type(4), TerrainType::Mountain);
        assert_eq!(m.terrain_type(1), TerrainType::Plains);
        assert_eq!(m.terrain_type(0), TerrainType::Ocean);
        assert!(m.is_water(0) && m.is_ocean(0));
    }

    #[test]
    fn ownership_and_border() {
        let mut m = map_3x3();
        assert!(!m.has_owner(4));
        assert!(m.set_owner_id(4, 7));
        assert_eq!(m.owner_id(4), 7);
        assert!(m.is_border(4));
        assert!(m.is_border(1));
        assert!(!m.is_border(8) || m.owner_id(5) != m.owner_id(8));
        assert!(!m.set_owner_id(4, 5000));
        assert_eq!(m.owner_id(4), 7);
    }

    #[test]
    fn edges_and_distances() {
        let m = map_3x3();
        assert!(m.is_on_edge_of_map(0));
        assert!(!m.is_on_edge_of_map(4));
        assert_eq!(m.manhattan_dist(0, 8), 4);
        assert_eq!(m.euclidean_dist_squared(0, 8), 8);
        assert_eq!(m.tile(2, 1), Some(5));
        assert_eq!(m.tile(3, 0), None);
    }
}
