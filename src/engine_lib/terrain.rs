//! laukofront's own terrain generator (not ported: upstream ships hand-made maps, see D20).
//!
//! 1. Fractal value noise (integer hash lattice, smoothstep interpolation, `octaves` layers).
//! 2. Elevation fades to zero near the map edge, so continents never touch it.
//! 3. Sea level is chosen from a histogram so about `land_fraction` of tiles are land.
//! 4. Land elevation above sea level becomes magnitude 0-30 (`detmath::pow` curve).
//! 5. The map style shapes it: one big continent (pangaea), many islands (archipelago), lakes.
//! 6. Rivers run downhill from the highlands into the sea; the highest peaks become impassable.
//! 7. Land specks smaller than `min_island_tiles` sink; lakes smaller than `min_lake_tiles` fill.
//! 8. Water connected to the map edge is ocean; tiles next to the other kind are shoreline.
//! 9. Biomes (colours only) from latitude, height and a moisture noise.
//!
//! Only integer hashing, `+ - * /` and `detmath` are used, so a seed gives the same map everywhere.

use std::collections::VecDeque;

use super::detmath;
use super::game_map::{terrain_byte, Biome, GameMap, IMPASSABLE_MAGNITUDE};
use super::random::PseudoRandom;

/// How far elevation drops at the very edge (noise is 0..1, so this sinks the rim).
const EDGE_DROP: f64 = 0.45;

/// The overall shape of a generated map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MapStyle {
    /// A few large continents (the default).
    #[default]
    Continents,
    /// Many islands and narrow seas: boats matter.
    Archipelago,
    /// One big landmass.
    Pangaea,
    /// Mostly land, full of lakes.
    Lakes,
}

impl MapStyle {
    pub const ALL: [MapStyle; 4] = [
        MapStyle::Continents,
        MapStyle::Archipelago,
        MapStyle::Pangaea,
        MapStyle::Lakes,
    ];

    pub fn name(self) -> &'static str {
        match self {
            MapStyle::Continents => "continents",
            MapStyle::Archipelago => "archipelago",
            MapStyle::Pangaea => "pangaea",
            MapStyle::Lakes => "lakes",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.to_ascii_lowercase();
        Self::ALL.into_iter().find(|m| m.name() == s)
    }

    /// Rivers per 400 000 tiles.
    fn rivers(self) -> u32 {
        match self {
            MapStyle::Continents => 6,
            MapStyle::Archipelago => 2,
            MapStyle::Pangaea => 9,
            MapStyle::Lakes => 4,
        }
    }
}

/// Generator settings. Defaults are laukofront's tuned values (`[ours]` in PLAN.md).
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainParams {
    pub width: u32,
    pub height: u32,
    /// Target share of tiles that are land, 0..1.
    pub land_fraction: f64,
    /// Noise layers; each adds detail at twice the frequency and half the weight.
    pub octaves: u32,
    /// Noise cells across the shorter map side for the first octave (bigger = more, smaller continents).
    pub base_cells: f64,
    /// Width of the band (share of the half-map, 0..1) over which elevation drops toward the edge.
    pub edge_fade: f64,
    /// Exponent of the elevation -> magnitude curve (bigger = more plains, fewer mountains).
    pub mountain_exponent: f64,
    pub min_island_tiles: u32,
    pub min_lake_tiles: u32,
    pub style: MapStyle,
    /// Share of land at the very top that becomes impassable peaks.
    pub peak_share: f64,
}

impl TerrainParams {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            land_fraction: 0.5,
            octaves: 6,
            base_cells: 4.0,
            edge_fade: 0.45,
            mountain_exponent: 1.6,
            min_island_tiles: 60,
            min_lake_tiles: 40,
            style: MapStyle::Continents,
            peak_share: 0.006,
        }
    }

    /// These settings adjusted for `style`.
    fn styled(&self) -> Self {
        let mut p = self.clone();
        match self.style {
            MapStyle::Continents => {}
            MapStyle::Archipelago => {
                p.land_fraction = self.land_fraction * 0.7;
                p.base_cells = self.base_cells * 3.0;
                p.min_island_tiles = self.min_island_tiles / 2;
            }
            MapStyle::Pangaea => {
                p.land_fraction = self.land_fraction * 0.9;
                p.base_cells = self.base_cells * 0.6;
                p.edge_fade = (self.edge_fade * 1.4).min(0.9);
            }
            MapStyle::Lakes => {
                p.land_fraction = (self.land_fraction * 1.3).min(0.8);
                p.min_lake_tiles = (self.min_lake_tiles / 3).max(8);
            }
        }
        p
    }
}

/// Generates a map. Returns `None` for an empty size.
pub fn generate(params: &TerrainParams, seed: i64) -> Option<GameMap> {
    let p = params.styled();
    let (w, h) = (p.width as usize, p.height as usize);
    if w < 3 || h < 3 {
        return None;
    }
    let mut rng = PseudoRandom::new(seed);
    let salts: Vec<u32> = (0..p.octaves.max(1))
        .map(|_| (rng.next() * 4_294_967_296.0) as u32)
        .collect();

    // 1-2. Elevation minus a rounded edge falloff. The falloff blends the square distance to the
    // edge with the round one, so continents get rounded corners instead of following the frame.
    let short = w.min(h) as f64;
    let scale = short / p.base_cells.max(0.5);
    let band = p.edge_fade.clamp(0.01, 1.0);
    let mut elev = vec![0f64; w * h];
    for y in 0..h {
        for x in 0..w {
            let e = fbm(x as f64 / scale, y as f64 / scale, &salts);
            let dx = (2.0 * x as f64 / (w - 1) as f64 - 1.0).abs();
            let dy = (2.0 * y as f64 / (h - 1) as f64 - 1.0).abs();
            let round = (dx * dx + dy * dy).sqrt() / std::f64::consts::SQRT_2;
            let d = 0.5 * dx.max(dy) + 0.5 * round;
            let falloff = smoothstep((d - (1.0 - band)) / band);
            elev[y * w + x] = e - EDGE_DROP * falloff;
        }
    }

    // 3. Sea level from a histogram.
    const BINS: usize = 1 << 14;
    let mut hist = vec![0u32; BINS];
    for &e in &elev {
        hist[bin(e, BINS)] += 1;
    }
    let want_land = (p.land_fraction.clamp(0.0, 1.0) * (w * h) as f64) as u64;
    let mut acc = 0u64;
    let mut sea_bin = BINS;
    while sea_bin > 0 && acc < want_land {
        sea_bin -= 1;
        acc += hist[sea_bin] as u64;
    }
    let sea = sea_bin as f64 / BINS as f64;
    let top = elev.iter().cloned().fold(0.0, f64::max).max(sea + 1e-9);

    // 4. Land and magnitude.
    let mut land: Vec<bool> = elev.iter().map(|&e| bin(e, BINS) >= sea_bin).collect();
    let mut mag: Vec<u8> = elev
        .iter()
        .map(|&e| {
            let hn = ((e - sea) / (top - sea)).clamp(0.0, 1.0);
            ((detmath::pow(hn, p.mountain_exponent) * 31.0) as u8).min(30)
        })
        .collect();

    // The outermost ring is always water, so the ocean surrounds the map.
    for (i, l) in land.iter_mut().enumerate() {
        let (x, y) = (i % w, i / w);
        if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            *l = false;
        }
    }

    // 5. Style.
    match p.style {
        MapStyle::Pangaea => keep_largest_land(&mut land, w, h),
        MapStyle::Lakes => {
            // A second, finer noise field punches lakes into the land.
            let lake_salts = [
                (rng.next() * 4_294_967_296.0) as u32,
                (rng.next() * 4_294_967_296.0) as u32,
            ];
            let ls = scale / 4.0;
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    if land[i]
                        && mag[i] < 14
                        && fbm(x as f64 / ls, y as f64 / ls, &lake_salts) > 0.68
                    {
                        land[i] = false;
                    }
                }
            }
        }
        _ => {}
    }

    // 6. Rivers, then peaks.
    let area = (w * h) as u64;
    let rivers = (p.style.rivers() as u64 * area / 400_000).max(1) as usize;
    carve_rivers(&mut land, &elev, w, h, rivers, &mut rng);
    mark_peaks(&land, &elev, &mut mag, p.peak_share);

    // 7-8.
    let terrain = finalize(
        &mut land,
        &mut mag,
        w,
        h,
        p.min_island_tiles,
        p.min_lake_tiles,
    );
    let mut map = GameMap::new(p.width, p.height, terrain)?;
    // Generated maps are drawn as if they spanned 70 N to 70 S.
    assign_biomes(&mut map, 70.0, -70.0, seed);
    Some(map)
}

/// Steps 7-8, shared with the real-Earth maps: sinks small islands, fills small lakes, and packs
/// the terrain bytes (land, shoreline, ocean, magnitude).
pub fn finalize(
    land: &mut [bool],
    mag: &mut [u8],
    w: usize,
    h: usize,
    min_island: u32,
    min_lake: u32,
) -> Vec<u8> {
    remove_small_components(land, w, h, true, min_island, |_| {});
    remove_small_components(land, w, h, false, min_lake, |i| mag[i] = 2);
    for (i, l) in land.iter().enumerate() {
        if !l {
            mag[i] = 0;
        }
    }
    let ocean = edge_connected_water(land, w, h);
    let mut terrain = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let shore = neighbors(i, w, h)
                .into_iter()
                .flatten()
                .any(|n| land[n] != land[i]);
            terrain[i] = terrain_byte(land[i], shore, ocean[i], mag[i]);
        }
    }
    terrain
}

/// Magnitudes 0-30 for land from our noise (used by the real-Earth maps, whose source has no
/// height): mostly plains, highlands and mountain ranges where the noise peaks.
pub fn noise_magnitudes(w: usize, h: usize, seed: i64, cells: f64) -> Vec<u8> {
    let mut rng = PseudoRandom::new(seed);
    let salts: Vec<u32> = (0..6)
        .map(|_| (rng.next() * 4_294_967_296.0) as u32)
        .collect();
    let scale = w.min(h) as f64 / cells.max(0.5);
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let e = fbm(x as f64 / scale, y as f64 / scale, &salts);
            // Ridges: values far from the middle become high ground.
            let r = ((e - 0.5).abs() * 2.0).clamp(0.0, 1.0);
            out[y * w + x] = ((detmath::pow(r, 1.8) * 31.0) as u8).min(30);
        }
    }
    out
}

/// Keeps only the biggest landmass.
fn keep_largest_land(land: &mut [bool], w: usize, h: usize) {
    let mut comp_id = vec![u32::MAX; land.len()];
    let mut sizes: Vec<u32> = Vec::new();
    let mut queue = VecDeque::new();
    for start in 0..land.len() {
        if !land[start] || comp_id[start] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        let mut n = 0;
        comp_id[start] = id;
        queue.push_back(start);
        while let Some(i) = queue.pop_front() {
            n += 1;
            for nb in neighbors(i, w, h).into_iter().flatten() {
                if land[nb] && comp_id[nb] == u32::MAX {
                    comp_id[nb] = id;
                    queue.push_back(nb);
                }
            }
        }
        sizes.push(n);
    }
    let Some(best) = (0..sizes.len()).max_by_key(|&i| (sizes[i], std::cmp::Reverse(i))) else {
        return;
    };
    for (i, l) in land.iter_mut().enumerate() {
        if *l && comp_id[i] != best as u32 {
            *l = false;
        }
    }
}

/// Rivers: from random high land, walk to the lowest unvisited neighbour until reaching water;
/// a walk that gets stuck or runs too long is dropped. Carved tiles become water.
fn carve_rivers(
    land: &mut [bool],
    elev: &[f64],
    w: usize,
    h: usize,
    count: usize,
    rng: &mut PseudoRandom,
) {
    let high: Vec<usize> = (0..land.len()).filter(|&i| land[i]).collect();
    if high.is_empty() {
        return;
    }
    // Sources: the highest of a few random land tiles, so rivers start in the hills.
    let max_len = (w + h) / 2;
    let mut made = 0;
    let mut tries = 0;
    let mut visited = vec![false; land.len()];
    while made < count && tries < count * 20 {
        tries += 1;
        let mut src = high[rng.next_int(0, high.len() as i64) as usize];
        for _ in 0..8 {
            let c = high[rng.next_int(0, high.len() as i64) as usize];
            if elev[c] > elev[src] {
                src = c;
            }
        }
        let mut path = vec![src];
        let mut cur = src;
        let mut reached = false;
        visited[src] = true;
        while path.len() < max_len {
            let next = neighbors(cur, w, h)
                .into_iter()
                .flatten()
                .filter(|&n| !visited[n])
                .min_by(|&a, &b| elev[a].total_cmp(&elev[b]));
            let Some(n) = next else { break };
            if !land[n] {
                reached = true;
                break;
            }
            visited[n] = true;
            path.push(n);
            cur = n;
        }
        for &i in &path {
            visited[i] = false;
        }
        // Short rivers are just notches in the coast; skip them.
        if reached && path.len() >= 25 {
            for &i in &path {
                land[i] = false;
            }
            made += 1;
        }
    }
}

/// The highest `share` of land becomes impassable.
fn mark_peaks(land: &[bool], elev: &[f64], mag: &mut [u8], share: f64) {
    if share <= 0.0 {
        return;
    }
    let mut heights: Vec<f64> = (0..land.len())
        .filter(|&i| land[i])
        .map(|i| elev[i])
        .collect();
    if heights.is_empty() {
        return;
    }
    let k = ((heights.len() as f64 * share) as usize).min(heights.len() - 1);
    let idx = heights.len() - 1 - k;
    heights.select_nth_unstable_by(idx, |a, b| a.total_cmp(b));
    let cut = heights[idx];
    for i in 0..land.len() {
        if land[i] && elev[i] > cut {
            mag[i] = IMPASSABLE_MAGNITUDE;
        }
    }
}

/// Biomes from latitude (`lat_top` at row 0 to `lat_bottom` at the last row), height and a
/// moisture noise. Visual only.
pub fn assign_biomes(map: &mut GameMap, lat_top: f64, lat_bottom: f64, seed: i64) {
    let (w, h) = (map.width() as usize, map.height() as usize);
    let mut rng = PseudoRandom::new(seed ^ 0x5eed_b10e);
    let salts: Vec<u32> = (0..3)
        .map(|_| (rng.next() * 4_294_967_296.0) as u32)
        .collect();
    let jitter_salts: Vec<u32> = (0..3)
        .map(|_| (rng.next() * 4_294_967_296.0) as u32)
        .collect();
    let scale = w.min(h) as f64 / 6.0;
    let jscale = w.min(h) as f64 / 10.0;
    // Degrees per row, so the jitter is the same in degrees on every map.
    let span = (lat_top - lat_bottom).abs().max(1.0);
    let mut out = vec![Biome::Temperate as u8; w * h];
    for y in 0..h {
        let row_lat = lat_top + (lat_bottom - lat_top) * (y as f64 + 0.5) / h as f64;
        for x in 0..w {
            let t = (y * w + x) as u32;
            if !map.is_land(t) {
                continue;
            }
            // Wavy climate lines instead of straight bands (up to +-9 degrees).
            let j = (fbm(x as f64 / jscale, y as f64 / jscale, &jitter_salts) - 0.5)
                * 18.0
                * span.min(40.0)
                / 40.0;
            let lat = (row_lat + j).abs();
            // Earth-like climate: a dry belt around 25 degrees, wet tropics near the equator.
            let dry = (1.0 - ((lat - 25.0) / 9.0) * ((lat - 25.0) / 9.0)).max(0.0);
            let tropic = (1.0 - lat / 10.0).max(0.0);
            let wet = fbm(x as f64 / scale, y as f64 / scale, &salts) - 0.2 * dry + 0.15 * tropic;
            let m = map.magnitude(t);
            let b = if lat > 66.0 || (lat > 58.0 && m >= 20) {
                if lat > 75.0 || m >= 20 {
                    Biome::Snow
                } else {
                    Biome::Tundra
                }
            } else if lat > 50.0 {
                Biome::Taiga
            } else if lat > 30.0 {
                if wet < 0.42 {
                    Biome::Steppe
                } else if wet > 0.58 {
                    Biome::Forest
                } else {
                    Biome::Temperate
                }
            } else if wet < 0.40 && m < 12 {
                Biome::Desert
            } else if wet > 0.52 {
                Biome::Jungle
            } else {
                Biome::Steppe
            };
            out[t as usize] = b as u8;
        }
    }
    map.set_biomes(out);
}

#[inline]
fn bin(e: f64, bins: usize) -> usize {
    ((e.clamp(0.0, 1.0) * bins as f64) as usize).min(bins - 1)
}

#[inline]
fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Lattice value in [0, 1) from a 32-bit integer hash (murmur3 finaliser).
#[inline]
fn lattice(ix: i64, iy: i64, salt: u32) -> f64 {
    let mut h =
        (ix as u32).wrapping_mul(0x8da6_b343) ^ (iy as u32).wrapping_mul(0xd816_3841) ^ salt;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h as f64 / 4_294_967_296.0
}

fn value_noise(x: f64, y: f64, salt: u32) -> f64 {
    let (x0, y0) = (x.floor(), y.floor());
    let (ix, iy) = (x0 as i64, y0 as i64);
    let (sx, sy) = (smoothstep(x - x0), smoothstep(y - y0));
    let a = lattice(ix, iy, salt);
    let b = lattice(ix + 1, iy, salt);
    let c = lattice(ix, iy + 1, salt);
    let d = lattice(ix + 1, iy + 1, salt);
    let top = a + (b - a) * sx;
    let bottom = c + (d - c) * sx;
    top + (bottom - top) * sy
}

fn fbm(x: f64, y: f64, salts: &[u32]) -> f64 {
    let (mut sum, mut amp, mut freq, mut total) = (0.0, 1.0, 1.0, 0.0);
    for &s in salts {
        sum += value_noise(x * freq, y * freq, s) * amp;
        total += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / total
}

#[inline]
fn neighbors(i: usize, w: usize, h: usize) -> [Option<usize>; 4] {
    let (x, y) = (i % w, i / w);
    [
        (y > 0).then(|| i - w),
        (y + 1 < h).then(|| i + w),
        (x > 0).then(|| i - 1),
        (x + 1 < w).then(|| i + 1),
    ]
}

/// Flips every 4-connected component of `kind` smaller than `min` (calls `on_flip` per tile).
fn remove_small_components(
    land: &mut [bool],
    w: usize,
    h: usize,
    kind: bool,
    min: u32,
    mut on_flip: impl FnMut(usize),
) {
    let mut seen = vec![false; land.len()];
    let mut queue = VecDeque::new();
    let mut comp = Vec::new();
    for start in 0..land.len() {
        if seen[start] || land[start] != kind {
            continue;
        }
        comp.clear();
        seen[start] = true;
        queue.push_back(start);
        let mut touches_edge = false;
        while let Some(i) = queue.pop_front() {
            comp.push(i);
            let (x, y) = (i % w, i / w);
            touches_edge |= x == 0 || y == 0 || x == w - 1 || y == h - 1;
            for n in neighbors(i, w, h).into_iter().flatten() {
                if !seen[n] && land[n] == kind {
                    seen[n] = true;
                    queue.push_back(n);
                }
            }
        }
        // Water touching the edge is the ocean: never fill it.
        let keep = comp.len() as u32 >= min || (!kind && touches_edge);
        if !keep {
            for &i in &comp {
                land[i] = !kind;
                on_flip(i);
            }
        }
    }
}

fn edge_connected_water(land: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut ocean = vec![false; land.len()];
    let mut queue = VecDeque::new();
    for i in 0..land.len() {
        let (x, y) = (i % w, i / w);
        if !land[i] && (x == 0 || y == 0 || x == w - 1 || y == h - 1) {
            ocean[i] = true;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        for n in neighbors(i, w, h).into_iter().flatten() {
            if !ocean[n] && !land[n] {
                ocean[n] = true;
                queue.push_back(n);
            }
        }
    }
    ocean
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_lib::game_map::TerrainType;

    fn small(seed: i64) -> GameMap {
        generate(&TerrainParams::new(240, 150), seed).unwrap()
    }

    #[test]
    fn same_seed_same_map() {
        assert_eq!(small(5).terrain_bytes(), small(5).terrain_bytes());
        assert_ne!(small(5).terrain_bytes(), small(6).terrain_bytes());
    }

    #[test]
    fn land_share_is_near_target() {
        let m = small(11);
        let share = m.num_land_tiles() as f64 / m.num_tiles() as f64;
        assert!((0.45..0.62).contains(&share), "land share {share}");
    }

    #[test]
    fn map_edge_is_ocean() {
        let m = small(2);
        for x in 0..m.width() {
            for t in [
                m.tile(x as i64, 0).unwrap(),
                m.tile(x as i64, m.height() as i64 - 1).unwrap(),
            ] {
                assert!(m.is_ocean(t));
            }
        }
    }

    #[test]
    fn shoreline_bits_are_consistent() {
        let m = small(3);
        let mut buf = [0; 4];
        for t in 0..m.num_tiles() {
            let n = m.neighbors4(t, &mut buf);
            let mixed = buf[..n].iter().any(|&nb| m.is_land(nb) != m.is_land(t));
            assert_eq!(m.is_shoreline(t), mixed, "tile {t}");
        }
    }

    #[test]
    fn has_all_land_types() {
        let m = small(4);
        let mut seen = [false; 3];
        for t in 0..m.num_tiles() {
            match m.terrain_type(t) {
                TerrainType::Plains => seen[0] = true,
                TerrainType::Highland => seen[1] = true,
                TerrainType::Mountain => seen[2] = true,
                _ => {}
            }
        }
        assert_eq!(seen, [true; 3]);
    }
}
