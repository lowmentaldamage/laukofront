//! HUD and on-map UI (P6). Everything here only reads the game; actions go through
//! `PendingIntents`, `ClientSettings` or `RestartRequested`.
//!
//! - `hud`: left column (troops, gold, land, attacks, leaderboard, notices), top-right clock and
//!   speed buttons, hover info, help card, attribution
//! - `minimap`: bottom-right overview with the camera rectangle; click to jump
//! - `tutorial`: step-by-step hints next to your land
//! - `labels`: player names and troop counts on the map, the "YOU" marker
//! - `endscreen`: victory / defeat panel with restart

pub mod buildbar;
pub mod endscreen;
pub mod hud;
pub mod labels;
pub mod minimap;
pub mod preview;
pub mod tutorial;

use bevy::picking::hover::HoverMap;
use bevy::prelude::*;

use crate::engine::{rules, Game, PlayerId, HUMAN};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            hud::HudPlugin,
            minimap::MinimapPlugin,
            tutorial::TutorialPlugin,
            labels::LabelsPlugin,
            endscreen::EndScreenPlugin,
            buildbar::BuildBarPlugin,
            preview::PreviewPlugin,
        ));
    }
}

pub const PANEL: Color = Color::srgba(0.05, 0.06, 0.09, 0.95);
pub const PANEL_BORDER: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);
pub const TEXT: Color = Color::srgb(0.92, 0.94, 0.97);
pub const TEXT_DIM: Color = Color::srgb(0.62, 0.66, 0.72);
pub const ACCENT: Color = Color::srgb(1.0, 0.84, 0.36);
pub const GOOD: Color = Color::srgb(0.45, 0.85, 0.5);
pub const DANGER: Color = Color::srgb(1.0, 0.42, 0.38);
pub const TROOPS_BLUE: Color = Color::srgb(0.32, 0.6, 1.0);

pub fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

/// True when the mouse is over any HUD node (so map clicks and scrolls should be ignored).
pub fn pointer_over_ui(hover: &HoverMap, nodes: &Query<(), With<Node>>) -> bool {
    hover
        .values()
        .any(|hits| hits.keys().any(|e| nodes.get(*e).is_ok()))
}

pub fn fmt_num(n: f64) -> String {
    let a = n.abs();
    if a >= 1_000_000.0 {
        format!("{:.2}M", n / 1_000_000.0)
    } else if a >= 10_000.0 {
        format!("{:.1}K", n / 1_000.0)
    } else {
        format!("{n:.0}")
    }
}

/// Numbers shown in the HUD, computed from the game. Pure so the tests can check them.
#[derive(Debug, Clone, PartialEq)]
pub struct HudNumbers {
    pub troops: f64,
    pub cap: f64,
    pub growth_per_sec: f64,
    pub gold: f64,
    pub gold_per_sec: f64,
    pub land_share: f64,
    pub tiles: u32,
    pub win_share: f64,
}

pub fn hud_numbers(game: &Game) -> Option<HudNumbers> {
    let me = game.player(HUMAN)?;
    let cfg = game.config();
    let hz = cfg.game.tick_hz as f64;
    let land = game.map().num_land_tiles().max(1) as f64;
    let alive = me.is_alive() && !game.in_spawn_phase();
    Some(HudNumbers {
        troops: me.troops() as f64,
        cap: rules::max_troops(cfg, me.kind(), me.num_tiles()),
        growth_per_sec: if alive {
            rules::troop_increase_rate(cfg, me) * hz
        } else {
            0.0
        },
        gold: me.gold() as f64,
        gold_per_sec: if alive {
            rules::gold_addition_rate(cfg, me.kind()) as f64 * hz
        } else {
            0.0
        },
        land_share: me.num_tiles() as f64 / land,
        tiles: me.num_tiles(),
        win_share: cfg.game.win_percent as f64 / 100.0,
    })
}

/// One leaderboard row.
#[derive(Debug, Clone, PartialEq)]
pub struct LeaderRow {
    pub rank: usize,
    pub id: PlayerId,
    pub name: String,
    pub land_share: f64,
    pub troops: f64,
    pub gold: f64,
    pub is_me: bool,
}

/// Top `n` living players by land (ties: lower id first). The human is appended if not in the top.
pub fn leaderboard(game: &Game, n: usize) -> Vec<LeaderRow> {
    let land = game.map().num_land_tiles().max(1) as f64;
    let mut players: Vec<_> = game.alive_players().collect();
    players.sort_by(|a, b| b.num_tiles().cmp(&a.num_tiles()).then(a.id().cmp(&b.id())));
    let row = |i: usize, p: &crate::engine::Player| LeaderRow {
        rank: i + 1,
        id: p.id(),
        name: p.name().to_string(),
        land_share: p.num_tiles() as f64 / land,
        troops: p.troops() as f64,
        gold: p.gold() as f64,
        is_me: p.id() == HUMAN,
    };
    let mut out: Vec<LeaderRow> = players
        .iter()
        .take(n)
        .enumerate()
        .map(|(i, p)| row(i, p))
        .collect();
    if !out.iter().any(|r| r.is_me) {
        if let Some((i, p)) = players.iter().enumerate().find(|(_, p)| p.id() == HUMAN) {
            out.push(row(i, p));
        }
    }
    out
}

/// One line of the attacks card.
#[derive(Debug, Clone, PartialEq)]
pub struct AttackLine {
    /// Attack id, or unit id for boats.
    pub attack: u32,
    /// A transport ship rather than a land attack.
    pub boat: bool,
    pub other: String,
    pub troops: f64,
    pub outgoing: bool,
}

pub fn attack_lines(game: &Game) -> Vec<AttackLine> {
    let mut out = Vec::new();
    let Some(me) = game.player(HUMAN) else {
        return out;
    };
    let name = |id: Option<PlayerId>| {
        id.and_then(|i| game.player(i))
            .map_or("free land".to_string(), |p| p.name().to_string())
    };
    for &a in me.outgoing_attacks() {
        if let Some(at) = game.attack(a) {
            out.push(AttackLine {
                attack: a,
                boat: false,
                other: name(at.target()),
                troops: at.troops(),
                outgoing: true,
            });
        }
    }
    for u in game.player_units(HUMAN, Some(crate::engine::units::UnitKind::TransportShip)) {
        let target = u.target_tile().and_then(|t| game.owner(t));
        out.push(AttackLine {
            attack: u.id(),
            boat: true,
            other: if u.is_retreating() {
                "home".to_string()
            } else {
                name(target)
            },
            troops: u.troops(),
            outgoing: true,
        });
    }
    for a in game.incoming_attacks(HUMAN) {
        if let Some(at) = game.attack(a) {
            out.push(AttackLine {
                attack: a,
                boat: false,
                other: name(Some(at.attacker())),
                troops: at.troops(),
                outgoing: false,
            });
        }
    }
    out
}

/// "mm:ss" game clock.
pub fn clock(game: &Game) -> String {
    let s = game.elapsed_game_seconds() as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
    use crate::engine::util::spawn_tiles;
    use crate::engine::{new_match, run_turn, Intent, MatchSetup};

    fn played(ticks: u32) -> Game {
        let cfg = GameConfig::load(GAME_CONFIG_PATH).unwrap();
        let mut g = new_match(
            cfg,
            MatchSetup {
                size: MapSize::Small,
                seed: 2,
                bots: 6,
            },
        )
        .unwrap();
        run_turn(&mut g, &[]);
        let map = g.map();
        let t = (0..map.num_tiles())
            .step_by(97)
            .find(|&t| spawn_tiles(map, t, 4, true).is_some())
            .unwrap();
        run_turn(
            &mut g,
            &[Intent::Spawn {
                player: HUMAN,
                tile: t,
            }],
        );
        for _ in 0..ticks {
            run_turn(&mut g, &[]);
        }
        g
    }

    #[test]
    fn hud_numbers_match_the_game() {
        let g = played(200);
        let me = g.player(HUMAN).unwrap();
        let h = hud_numbers(&g).unwrap();
        assert_eq!(h.troops, me.troops() as f64);
        assert_eq!(h.gold, me.gold() as f64);
        assert_eq!(h.tiles, me.num_tiles());
        assert_eq!(h.gold_per_sec, 1000.0, "100 gold per tick at 10 Hz");
        assert_eq!(
            h.cap,
            rules::max_troops(g.config(), me.kind(), me.num_tiles())
        );
        assert!(
            (h.growth_per_sec - rules::troop_increase_rate(g.config(), me) * 10.0).abs() < 1e-9
        );
        assert_eq!(h.win_share, 0.8);
    }

    #[test]
    fn leaderboard_is_sorted_and_includes_me() {
        let g = played(300);
        let rows = leaderboard(&g, 3);
        assert!(rows
            .windows(2)
            .all(|w| w[0].land_share >= w[1].land_share || w[1].is_me));
        assert!(rows.iter().any(|r| r.is_me));
        assert_eq!(rows[0].rank, 1);
    }

    #[test]
    fn attack_lines_list_outgoing_attacks() {
        let mut g = played(10);
        run_turn(
            &mut g,
            &[Intent::Attack {
                player: HUMAN,
                target: None,
                troops: Some(1_000.0),
            }],
        );
        let lines = attack_lines(&g);
        assert!(lines.iter().any(|l| l.outgoing && l.other == "free land"));
    }

    #[test]
    fn number_format() {
        assert_eq!(fmt_num(950.0), "950");
        assert_eq!(fmt_num(25_000.0), "25.0K");
        assert_eq!(fmt_num(1_234_567.0), "1.23M");
        assert_eq!(fmt_num(-12_000.0), "-12.0K");
    }
}
