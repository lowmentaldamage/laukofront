//! Small helpers ported from upstream `packages/engine-lib/src/Util.ts` and
//! `packages/engine/src/execution/Util.ts`.

use crate::engine_lib::game_map::{GameMap, TileRef};

/// Upstream `simpleHash`: Java-style string hash over UTF-16 code units, 32-bit, absolute value.
pub fn simple_hash(s: &str) -> i64 {
    let mut h: i32 = 0;
    for c in s.encode_utf16() {
        h = (h << 5).wrapping_sub(h).wrapping_add(c as i32);
    }
    (h as i64).abs()
}

/// Upstream `getSpawnTiles`: every tile within distance `radius` of `center`, measured from the
/// corner between four tiles (`center - 0.5`), in breadth-first order from `center`.
/// With `require_all_valid`, returns `None` if any of them is owned, water or impassable;
/// otherwise returns only the valid ones.
pub fn spawn_tiles(
    map: &GameMap,
    center: TileRef,
    radius: u32,
    require_all_valid: bool,
) -> Option<Vec<TileRef>> {
    let r2 = (radius * radius) as f64;
    let (cx, cy) = (map.x(center) as f64 - 0.5, map.y(center) as f64 - 0.5);
    let inside = |t: TileRef| {
        let dx = map.x(t) as f64 - cx;
        let dy = map.y(t) as f64 - cy;
        dx * dx + dy * dy <= r2
    };
    let mut out = Vec::new();
    if inside(center) {
        out.push(center);
        let mut head = 0;
        let mut buf = [0; 4];
        while head < out.len() {
            let cur = out[head];
            head += 1;
            let n = map.neighbors4(cur, &mut buf);
            for &nb in &buf[..n] {
                if inside(nb) && !out.contains(&nb) {
                    out.push(nb);
                }
            }
        }
    }
    let invalid = |t: TileRef| map.has_owner(t) || !map.is_passable_land(t);
    if require_all_valid {
        if out.iter().any(|&t| invalid(t)) {
            return None;
        }
        Some(out)
    } else {
        out.retain(|&t| !invalid(t));
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_lib::game_map::terrain_byte;

    #[test]
    fn simple_hash_matches_java_string_hash() {
        assert_eq!(simple_hash(""), 0);
        assert_eq!(simple_hash("a"), 97);
        // "hello".hashCode() in Java = 99162322
        assert_eq!(simple_hash("hello"), 99_162_322);
        // Overflowing hash: Java "polygenelubricants".hashCode() = -2147483648 -> abs = 2^31
        assert_eq!(simple_hash("polygenelubricants"), 2_147_483_648);
    }

    #[test]
    fn spawn_disk_has_expected_size() {
        let map = GameMap::new(20, 20, vec![terrain_byte(true, false, false, 1); 400]).unwrap();
        let center = map.tile(10, 10).unwrap();
        let tiles = spawn_tiles(&map, center, 4, true).unwrap();
        // Disk of radius 4 around a tile corner: 52 tiles.
        assert_eq!(tiles.len(), 52);
        assert_eq!(tiles[0], center);
        assert!(tiles
            .iter()
            .all(|&t| map.euclidean_dist_squared(t, center) <= 32));
    }

    #[test]
    fn spawn_rejects_water_when_all_required() {
        let mut terrain = vec![terrain_byte(true, false, false, 1); 400];
        terrain[10 * 20 + 12] = terrain_byte(false, false, true, 0);
        let map = GameMap::new(20, 20, terrain).unwrap();
        let c = map.tile(10, 10).unwrap();
        assert!(spawn_tiles(&map, c, 4, true).is_none());
        assert_eq!(spawn_tiles(&map, c, 4, false).unwrap().len(), 51);
    }
}
