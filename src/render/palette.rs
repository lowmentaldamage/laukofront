//! laukofront's own colours (not OpenFront's). Pure functions so they can be tested.

use crate::engine::{PlayerId, HUMAN};
use crate::engine_lib::game_map::{Biome, GameMap, TerrainType, TileRef};

pub const OCEAN: [u8; 4] = [24, 44, 72, 255];
pub const LAKE: [u8; 4] = [38, 72, 104, 255];
pub const IMPASSABLE: [u8; 4] = [64, 64, 72, 255];
/// The human's colour.
pub const HUMAN_COLOR: [u8; 3] = [70, 140, 255];
/// Irradiated unowned land after a nuke.
pub const FALLOUT: [u8; 4] = [74, 82, 38, 255];
/// Border colour of the human's own land.
pub const OWN_BORDER: [u8; 4] = [255, 214, 92, 255];
/// How strongly an owner's colour covers the terrain (0..1).
pub const OWNER_TINT: f32 = 0.62;

/// Base colour of each biome's lowlands (ours).
fn biome_base(b: Biome) -> [u32; 3] {
    match b {
        Biome::Temperate => [104, 148, 84],
        Biome::Forest => [66, 118, 64],
        Biome::Steppe => [150, 156, 92],
        Biome::Desert => [206, 186, 128],
        Biome::Jungle => [52, 120, 58],
        Biome::Taiga => [74, 106, 80],
        Biome::Tundra => [150, 152, 128],
        Biome::Snow => [226, 232, 236],
    }
}

/// Terrain colour of a tile: its biome, darkening and greying with height.
pub fn terrain_color(map: &GameMap, t: TileRef) -> [u8; 4] {
    let m = map.magnitude(t) as u32;
    match map.terrain_type(t) {
        TerrainType::Ocean => OCEAN,
        TerrainType::Lake => LAKE,
        TerrainType::Impassable => IMPASSABLE,
        TerrainType::Plains => {
            let [r, g, b] = biome_base(map.biome(t));
            [
                (r + m * 2).min(255) as u8,
                g.saturating_sub(m).min(255) as u8,
                (b + m).min(255) as u8,
                255,
            ]
        }
        TerrainType::Highland => {
            let [r, g, b] = biome_base(map.biome(t));
            // Blend toward brown hills.
            let mix = |a: u32, hill: u32| ((a + hill * 2) / 3 + (m - 10)) as u8;
            [mix(r, 150), mix(g, 126), mix(b, 92), 255]
        }
        TerrainType::Mountain => {
            let snow = matches!(map.biome(t), Biome::Snow | Biome::Tundra | Biome::Taiga);
            let v = (150 + (m - 20) * 6).min(235) as u8;
            if snow {
                let v = v.saturating_add(20);
                [v, v, v.saturating_add(6), 255]
            } else {
                [v, v, v, 255]
            }
        }
    }
}

/// Team of each player id in a team match (`u8::MAX` = none), set when a match starts.
static TEAMS: std::sync::RwLock<Vec<u8>> = std::sync::RwLock::new(Vec::new());

/// Hues of teams 1-4: red, blue, green, orange.
const TEAM_HUES: [f32; 4] = [2.0, 215.0, 125.0, 35.0];

/// Remembers the teams of the current match so players are coloured by team.
pub fn set_teams(game: &crate::engine::Game) {
    let mut v = vec![u8::MAX; game.players().len() + 2];
    for p in game.players() {
        if let Some(t) = p.team() {
            v[p.id() as usize] = t;
        }
    }
    if let Ok(mut w) = TEAMS.write() {
        *w = v;
    }
}

/// A distinct colour per player: the human is blue; others spread around the hue circle by the
/// golden angle, skipping the blue range so nobody looks like the human. In a team match every
/// team member gets a shade of the team's colour.
pub fn player_color(id: PlayerId) -> [u8; 3] {
    let team = TEAMS
        .read()
        .ok()
        .and_then(|t| t.get(id as usize).copied())
        .filter(|&t| t != u8::MAX);
    if let Some(t) = team {
        let hue = (TEAM_HUES[(t as usize) % 4] + ((id % 5) as f32 - 2.0) * 7.0).rem_euclid(360.0);
        let sat = 0.55 + 0.1 * ((id % 3) as f32 / 2.0);
        let val = 0.72 + 0.2 * ((id % 4) as f32 / 3.0);
        return hsv_to_rgb(hue, sat, val);
    }
    if id == HUMAN {
        return HUMAN_COLOR;
    }
    let mut hue = (id as f32 * 137.508) % 360.0;
    if (165.0..270.0).contains(&hue) {
        hue = (hue + 105.0) % 360.0;
    }
    let sat = 0.55 + 0.15 * ((id % 3) as f32 / 2.0);
    let val = 0.80 + 0.12 * ((id % 2) as f32);
    hsv_to_rgb(hue, sat, val)
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let hp = (h / 60.0) % 6.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let to = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    [to(r), to(g), to(b)]
}

/// Final colour of a tile: terrain, tinted by its owner; owner border tiles are drawn solid and
/// darker so territories have crisp edges.
pub fn tile_color(map: &GameMap, t: TileRef) -> [u8; 4] {
    let base = terrain_color(map, t);
    let owner = map.owner_id(t);
    if owner == 0 {
        if map.has_fallout(t) {
            return FALLOUT;
        }
        return base;
    }
    let pc = player_color(owner);
    if map.is_border(t) {
        // Your own border is bright gold so your land stands out on any map.
        if owner == HUMAN {
            return OWN_BORDER;
        }
        return [
            (pc[0] as f32 * 0.55) as u8,
            (pc[1] as f32 * 0.55) as u8,
            (pc[2] as f32 * 0.55) as u8,
            255,
        ];
    }
    let mix = |a: u8, b: u8| (a as f32 * (1.0 - OWNER_TINT) + b as f32 * OWNER_TINT) as u8;
    [
        mix(base[0], pc[0]),
        mix(base[1], pc[1]),
        mix(base[2], pc[2]),
        255,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_lib::game_map::terrain_byte;

    #[test]
    fn human_is_blue_and_others_are_not() {
        assert_eq!(player_color(HUMAN), HUMAN_COLOR);
        for id in 2..300u16 {
            let [r, g, b] = player_color(id);
            let [hr, hg, hb] = HUMAN_COLOR;
            let d2 = (r as i32 - hr as i32).pow(2)
                + (g as i32 - hg as i32).pow(2)
                + (b as i32 - hb as i32).pow(2);
            let bluish = d2 < 90 * 90;
            assert!(!bluish, "player {id} looks like the human: {:?}", (r, g, b));
        }
    }

    #[test]
    fn neighbouring_ids_get_different_colors() {
        for id in 2..200u16 {
            assert_ne!(player_color(id), player_color(id + 1));
        }
    }

    #[test]
    fn owned_tiles_are_tinted_and_borders_darker() {
        let mut map = GameMap::new(3, 1, vec![terrain_byte(true, false, false, 2); 3]).unwrap();
        let plain = tile_color(&map, 1);
        map.set_owner_id(0, 2);
        map.set_owner_id(1, 2);
        map.set_owner_id(2, 2);
        let inner = tile_color(&map, 1);
        assert_ne!(plain, inner);
        map.set_owner_id(2, 3);
        let border = tile_color(&map, 1);
        assert!(
            border.iter().take(3).map(|&c| c as u32).sum::<u32>()
                < inner.iter().take(3).map(|&c| c as u32).sum::<u32>()
        );
    }
}
