//! Real-Earth maps (ours). Coastlines come from Natural Earth (public domain, see CREDITS), turned
//! into land masks by `src/bin/earth_import.rs` and stored in `assets/maps/*.lkmap`. Height and
//! biomes come from our own noise, as the source has no height data.
//!
//! File format (`LKMAP1`): magic, width and height (u32 LE), top and bottom latitude (f32 LE),
//! then run lengths (LEB128) alternating water and land, starting with water.

use super::game_map::GameMap;
use super::terrain::{assign_biomes, finalize, noise_magnitudes};

const MAGIC: &[u8; 6] = b"LKMAP1";

/// A real-Earth map shipped with the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EarthMap {
    World,
    Europe,
    Asia,
    Africa,
    Americas,
    BritishIsles,
}

/// Where a map's mask comes from: longitude and latitude bounds and size in tiles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EarthRegion {
    pub lon_west: f64,
    pub lon_east: f64,
    pub lat_north: f64,
    pub lat_south: f64,
    pub width: u32,
    pub height: u32,
}

impl EarthMap {
    pub const ALL: [EarthMap; 6] = [
        EarthMap::World,
        EarthMap::Europe,
        EarthMap::Asia,
        EarthMap::Africa,
        EarthMap::Americas,
        EarthMap::BritishIsles,
    ];

    /// Command-line and file name.
    pub fn id(self) -> &'static str {
        match self {
            EarthMap::World => "world",
            EarthMap::Europe => "europe",
            EarthMap::Asia => "asia",
            EarthMap::Africa => "africa",
            EarthMap::Americas => "americas",
            EarthMap::BritishIsles => "british-isles",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            EarthMap::World => "World",
            EarthMap::Europe => "Europe",
            EarthMap::Asia => "Asia",
            EarthMap::Africa => "Africa",
            EarthMap::Americas => "The Americas",
            EarthMap::BritishIsles => "British Isles",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.to_ascii_lowercase();
        Self::ALL.into_iter().find(|m| m.id() == s)
    }

    /// Bounds and size. Equirectangular; regional maps choose their size so a tile covers about
    /// the same distance east-west as north-south in the middle of the map.
    pub fn region(self) -> EarthRegion {
        let r = |lon_west, lon_east, lat_north, lat_south, width, height| EarthRegion {
            lon_west,
            lon_east,
            lat_north,
            lat_south,
            width,
            height,
        };
        match self {
            EarthMap::World => r(-180.0, 180.0, 80.0, -58.0, 2000, 766),
            EarthMap::Europe => r(-25.0, 45.0, 72.0, 34.0, 1200, 1000),
            EarthMap::Asia => r(25.0, 150.0, 78.0, -10.0, 1400, 1200),
            EarthMap::Africa => r(-20.0, 55.0, 38.0, -36.0, 1100, 1090),
            EarthMap::Americas => r(-170.0, -30.0, 75.0, -57.0, 1000, 1180),
            EarthMap::BritishIsles => r(-11.0, 2.5, 61.0, 49.5, 600, 890),
        }
    }

    /// Suggested (bots, nations) for a full match.
    pub fn suggested_players(self) -> (u32, u32) {
        match self {
            EarthMap::World => (50, 16),
            EarthMap::Europe => (25, 10),
            EarthMap::Asia => (30, 12),
            EarthMap::Africa => (25, 10),
            EarthMap::Americas => (25, 10),
            EarthMap::BritishIsles => (10, 4),
        }
    }

    /// The mask file, built into the game.
    pub fn bytes(self) -> &'static [u8] {
        match self {
            EarthMap::World => include_bytes!("../../assets/maps/world.lkmap"),
            EarthMap::Europe => include_bytes!("../../assets/maps/europe.lkmap"),
            EarthMap::Asia => include_bytes!("../../assets/maps/asia.lkmap"),
            EarthMap::Africa => include_bytes!("../../assets/maps/africa.lkmap"),
            EarthMap::Americas => include_bytes!("../../assets/maps/americas.lkmap"),
            EarthMap::BritishIsles => include_bytes!("../../assets/maps/british-isles.lkmap"),
        }
    }

    /// Builds the playable map. Heights are noise seeded by the map (the same every match).
    pub fn build(self) -> Option<GameMap> {
        let mask = decode(self.bytes())?;
        build_from_mask(&mask, seed_of(self.id()))
    }
}

/// A decoded mask.
#[derive(Debug, Clone, PartialEq)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    pub lat_north: f32,
    pub lat_south: f32,
    pub land: Vec<bool>,
}

fn seed_of(id: &str) -> i64 {
    id.bytes().fold(1_469_598_103_934_665_603u64, |h, b| {
        (h ^ b as u64).wrapping_mul(1_099_511_628_211)
    }) as i64
}

/// Turns a mask into a map: noise heights on land, small specks and ponds removed, shoreline and
/// ocean bits, biomes from the real latitudes.
pub fn build_from_mask(mask: &Mask, seed: i64) -> Option<GameMap> {
    let (w, h) = (mask.width as usize, mask.height as usize);
    let mut land = mask.land.clone();
    let mut mag = noise_magnitudes(w, h, seed, 7.0);
    let terrain = finalize(&mut land, &mut mag, w, h, 12, 20);
    let mut map = GameMap::new(mask.width, mask.height, terrain)?;
    assign_biomes(&mut map, mask.lat_north as f64, mask.lat_south as f64, seed);
    Some(map)
}

pub fn encode(mask: &Mask) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&mask.width.to_le_bytes());
    out.extend_from_slice(&mask.height.to_le_bytes());
    out.extend_from_slice(&mask.lat_north.to_le_bytes());
    out.extend_from_slice(&mask.lat_south.to_le_bytes());
    let mut cur = false;
    let mut run: u64 = 0;
    let push = |out: &mut Vec<u8>, mut n: u64| loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    };
    for &l in &mask.land {
        if l == cur {
            run += 1;
        } else {
            push(&mut out, run);
            cur = l;
            run = 1;
        }
    }
    push(&mut out, run);
    out
}

pub fn decode(bytes: &[u8]) -> Option<Mask> {
    if bytes.len() < 22 || &bytes[..6] != MAGIC {
        return None;
    }
    let u32_at =
        |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let f32_at =
        |i: usize| f32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let (width, height) = (u32_at(6), u32_at(10));
    let (lat_north, lat_south) = (f32_at(14), f32_at(18));
    let n = width as usize * height as usize;
    if width == 0 || height == 0 || n > 16_000_000 {
        return None;
    }
    let mut land = Vec::with_capacity(n);
    let mut cur = false;
    let mut i = 22;
    while i < bytes.len() {
        let mut v: u64 = 0;
        let mut shift = 0;
        loop {
            let b = *bytes.get(i)?;
            i += 1;
            v |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 56 {
                return None;
            }
        }
        if land.len() as u64 + v > n as u64 {
            return None;
        }
        land.extend(std::iter::repeat_n(cur, v as usize));
        cur = !cur;
    }
    (land.len() == n).then_some(Mask {
        width,
        height,
        lat_north,
        lat_south,
        land,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trip() {
        let land: Vec<bool> = (0..37 * 23).map(|i| (i * 7919) % 13 < 5).collect();
        let m = Mask {
            width: 37,
            height: 23,
            lat_north: 10.0,
            lat_south: -5.0,
            land,
        };
        assert_eq!(decode(&encode(&m)), Some(m));
    }

    #[test]
    fn corrupt_files_are_rejected() {
        let m = Mask {
            width: 4,
            height: 4,
            lat_north: 0.0,
            lat_south: 0.0,
            land: vec![true; 16],
        };
        let mut b = encode(&m);
        b[0] = b'X';
        assert_eq!(decode(&b), None);
        let mut b = encode(&m);
        b.push(5); // runs past the end
        assert_eq!(decode(&b), None);
        assert_eq!(decode(&encode(&m)[..21]), None);
    }

    #[test]
    fn every_shipped_map_builds() {
        for m in EarthMap::ALL {
            let r = m.region();
            let map = m
                .build()
                .unwrap_or_else(|| panic!("{} does not build", m.id()));
            assert_eq!(
                (map.width(), map.height()),
                (r.width, r.height),
                "{}",
                m.id()
            );
            let land = map.num_land_tiles() as f64 / map.num_tiles() as f64;
            assert!((0.08..0.9).contains(&land), "{}: land share {land}", m.id());
        }
    }
}
