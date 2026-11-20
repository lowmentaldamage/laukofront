//! HUD (P5): stats, leaderboard, notices, help line and the licence notice. Reads the game only.

use bevy::prelude::*;

use crate::client::{ClientSettings, GameState, Notices};
use crate::engine::{rules, Game, HUMAN};
use crate::input::Hovered;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, update_hud);
    }
}

#[derive(Component)]
struct StatsText;
#[derive(Component)]
struct LeaderText;
#[derive(Component)]
struct CenterText;
#[derive(Component)]
struct HoverText;

const PANEL: Color = Color::srgba(0.04, 0.05, 0.07, 0.82);

fn panel(node: Node) -> (Node, BackgroundColor) {
    (
        Node {
            padding: UiRect::all(px(8)),
            ..node
        },
        BackgroundColor(PANEL),
    )
}

fn spawn_hud(mut commands: Commands) {
    let font = |size: f32| TextFont {
        font_size: FontSize::Px(size),
        ..default()
    };
    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            top: px(8),
            left: px(8),
            ..default()
        }))
        .with_child((Text::new(""), font(16.0), StatsText));
    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            top: px(8),
            right: px(8),
            ..default()
        }))
        .with_child((Text::new(""), font(15.0), LeaderText));
    commands.spawn((
        Text::new(""),
        font(22.0),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: px(60),
            left: percent(25),
            width: percent(50),
            ..default()
        },
        CenterText,
    ));
    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            bottom: px(8),
            left: px(8),
            ..default()
        }))
        .with_child((Text::new(""), font(14.0), HoverText));
    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            bottom: px(8),
            right: px(8),
            ..default()
        }))
        .with_child((
            Text::new(
                "Click: spawn / attack   1-9: attack size   R: retreat   Space: pause   F1-F3: speed   WASD / right-drag / wheel: camera\n\
                 laukofront is based on OpenFront. Copyright (c) OpenFront and Contributors. Not affiliated with OpenFront.",
            ),
            font(13.0),
            TextColor(Color::srgb(0.75, 0.78, 0.82)),
        ));
}

fn fmt_num(n: f64) -> String {
    if n >= 1_000_000.0 {
        format!("{:.2}M", n / 1_000_000.0)
    } else if n >= 10_000.0 {
        format!("{:.1}K", n / 1_000.0)
    } else {
        format!("{n:.0}")
    }
}

/// The stats panel text. Pure so it can be tested.
pub fn stats_text(game: &Game, settings: &ClientSettings) -> String {
    let land = game.map().num_land_tiles().max(1) as f64;
    let clock = game.elapsed_game_seconds() as u64;
    let mut s = format!("Time {:02}:{:02}", clock / 60, clock % 60);
    if settings.paused {
        s.push_str("  PAUSED");
    } else if settings.speed > 1 {
        s.push_str(&format!("  x{}", settings.speed));
    }
    s.push('\n');
    if let Some(me) = game.player(HUMAN) {
        let cap = rules::max_troops(game.config(), me.kind(), me.num_tiles());
        s.push_str(&format!(
            "Troops {} / {}\nGold {}\nLand {:.1}%\nAttack size {:.0}%",
            fmt_num(me.troops() as f64),
            fmt_num(cap),
            fmt_num(me.gold() as f64),
            100.0 * me.num_tiles() as f64 / land,
            settings.attack_ratio * 100.0
        ));
        let attacks = me.outgoing_attacks().len();
        if attacks > 0 {
            s.push_str(&format!("\nAttacks {attacks}"));
        }
    }
    s
}

/// The leaderboard text: top players by land.
pub fn leaderboard_text(game: &Game, rows: usize) -> String {
    let land = game.map().num_land_tiles().max(1) as f64;
    let mut players: Vec<_> = game.alive_players().collect();
    players.sort_by(|a, b| b.num_tiles().cmp(&a.num_tiles()).then(a.id().cmp(&b.id())));
    let mut s = format!("Leaderboard ({} alive)\n", players.len());
    for (i, p) in players.iter().take(rows).enumerate() {
        let me = if p.id() == HUMAN { " <" } else { "" };
        s.push_str(&format!(
            "{}. {:<8} {:>5.1}%  {:>7}{me}\n",
            i + 1,
            p.name(),
            100.0 * p.num_tiles() as f64 / land,
            fmt_num(p.troops() as f64)
        ));
    }
    s.trim_end().to_string()
}

fn center_text(game: &Game, notices: &Notices) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(w) = game.winner() {
        let name = game.player(w).map_or("?", |p| p.name());
        lines.push(if w == HUMAN {
            "Victory! You own most of the land.".into()
        } else {
            format!("{name} has won.")
        });
    } else if game.in_spawn_phase() {
        let left = game
            .config()
            .game
            .spawn_phase_ticks
            .saturating_sub(game.ticks())
            / game.config().game.tick_hz as u64;
        lines.push(format!(
            "Click unclaimed land to choose your spawn ({left} s)"
        ));
    } else if game.player(HUMAN).is_some_and(|p| !p.is_alive()) {
        lines.push(if game.player(HUMAN).is_some_and(|p| p.has_spawned()) {
            "You have been eliminated".into()
        } else {
            "You did not spawn - watching the bots".into()
        });
    }
    lines.extend(notices.0.iter().map(|n| n.0.clone()));
    lines.join("\n")
}

fn hover_text(game: &Game, hovered: Option<u32>) -> String {
    let Some(t) = hovered else {
        return String::new();
    };
    let map = game.map();
    let terrain = format!("{:?}", map.terrain_type(t));
    match game.owner(t).and_then(|o| game.player(o)) {
        Some(p) => format!(
            "{terrain} - {} ({} tiles, {} troops)",
            p.name(),
            p.num_tiles(),
            fmt_num(p.troops() as f64)
        ),
        None if map.is_passable_land(t) => format!("{terrain} - unclaimed"),
        None => terrain,
    }
}

#[allow(clippy::type_complexity)]
fn update_hud(
    game: Res<GameState>,
    settings: Res<ClientSettings>,
    notices: Res<Notices>,
    hovered: Res<Hovered>,
    mut texts: ParamSet<(
        Query<&mut Text, With<StatsText>>,
        Query<&mut Text, With<LeaderText>>,
        Query<&mut Text, With<CenterText>>,
        Query<&mut Text, With<HoverText>>,
    )>,
) {
    let g = &game.0;
    let set = |t: &mut Text, s: String| {
        if t.0 != s {
            t.0 = s;
        }
    };
    for mut t in texts.p0().iter_mut() {
        set(&mut t, stats_text(g, &settings));
    }
    for mut t in texts.p1().iter_mut() {
        set(&mut t, leaderboard_text(g, 10));
    }
    for mut t in texts.p2().iter_mut() {
        set(&mut t, center_text(g, &notices));
    }
    for mut t in texts.p3().iter_mut() {
        set(&mut t, hover_text(g, hovered.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
    use crate::engine::{new_match, run_turn, MatchSetup};

    #[test]
    fn hud_numbers_match_the_game() {
        let cfg = GameConfig::load(GAME_CONFIG_PATH).unwrap();
        let mut g = new_match(
            cfg,
            MatchSetup {
                size: MapSize::Small,
                seed: 2,
                bots: 5,
            },
        )
        .unwrap();
        for _ in 0..300 {
            run_turn(&mut g, &[]);
        }
        let me = g.player(HUMAN).unwrap();
        let s = stats_text(&g, &ClientSettings::default());
        assert!(
            s.contains(&format!("Troops {}", fmt_num(me.troops() as f64))),
            "{s}"
        );
        assert!(s.contains("Attack size 20%"));
        let lb = leaderboard_text(&g, 10);
        let alive = g.alive_players().count();
        assert!(lb.starts_with(&format!("Leaderboard ({alive} alive)")));
        let top = g
            .alive_players()
            .max_by(|a, b| a.num_tiles().cmp(&b.num_tiles()).then(b.id().cmp(&a.id())))
            .unwrap();
        assert!(lb.lines().nth(1).unwrap().contains(top.name()), "{lb}");
    }

    #[test]
    fn number_format() {
        assert_eq!(fmt_num(950.0), "950");
        assert_eq!(fmt_num(25_000.0), "25.0K");
        assert_eq!(fmt_num(1_234_567.0), "1.23M");
    }
}
