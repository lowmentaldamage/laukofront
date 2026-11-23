//! Left-side HUD column, top-right clock with pause/speed buttons, spawn-phase banner, hover
//! info, the key-help card (H) and the always-visible attribution.
//!
//! Every changing piece of the HUD carries one tag component (`HudText`, `HudNode`, `HudBg`) that
//! says which value it shows, so a few small systems can update all of them.

use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

use super::{
    attack_lines, clock, fmt_num, font, hud_numbers, leaderboard, AttackLine, ACCENT, DANGER, GOOD,
    PANEL, PANEL_BORDER, TEXT, TEXT_DIM, TROOPS_BLUE,
};
use crate::client::{ClientSettings, GameState, MatchStats, NoticeKind, Notices, PendingIntents};
use crate::engine::{Intent, HUMAN};
use crate::input::Hovered;
use crate::render::palette;

const ATTACK_ROWS: usize = 6;
const LEADER_ROWS: usize = 11;
const NOTICE_ROWS: usize = 6;
const COLUMN_WIDTH: f32 = 300.0;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HudText {
    Troops,
    TroopsSub,
    Ratio,
    Gold,
    GoldSub,
    Land,
    LandSub,
    AttacksTitle,
    AttackRow(usize),
    LeaderTitle,
    LeaderRow(usize),
    Notice(usize),
    Clock,
    Pause,
    Phase,
    Hover,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HudNode {
    TroopsFill,
    LandFill,
    AttackRow(usize),
    LeaderRow(usize),
    AttacksCard,
    PhaseBanner,
    HelpCard,
    HoverPanel,
    NoticeRow(usize),
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HudBg {
    RatioSeg(usize),
    Speed(u32),
    Pause,
    LeaderSwatch(usize),
    LeaderRow(usize),
}

/// What each attack row's retreat button cancels: (id, is_boat).
#[derive(Resource, Default)]
pub struct AttackRowIds(pub Vec<Option<(u32, bool)>>);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AttackRowIds>()
            .add_systems(Startup, spawn_hud)
            .add_systems(Update, (update_texts, update_nodes, update_backgrounds));
    }
}

fn card() -> impl Bundle {
    (
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(px(10), px(8)),
            row_gap: px(4),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(PANEL_BORDER),
    )
}

fn label(tag: HudText, size: f32, color: Color) -> impl Bundle {
    (Text::new(""), font(size), TextColor(color), tag)
}

fn bar(fill: HudNode, color: Color) -> impl Bundle {
    (
        Node {
            width: percent(100),
            height: px(8),
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
        children![(
            Node {
                width: percent(0),
                height: percent(100),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BackgroundColor(color),
            fill,
        )],
    )
}

fn small_button(text: &str, tag: HudBg) -> impl Bundle {
    (
        Button,
        Node {
            padding: UiRect::axes(px(8), px(3)),
            border_radius: BorderRadius::all(px(4)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
        tag,
        children![(Text::new(text), font(14.0), TextColor(TEXT))],
    )
}

fn spawn_hud(mut commands: Commands) {
    // ---- left column ----
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: px(8),
            left: px(8),
            width: px(COLUMN_WIDTH),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        })
        .with_children(|col| {
            // Troops + attack size
            col.spawn(card()).with_children(|c| {
                c.spawn(label(HudText::Troops, 17.0, TEXT));
                c.spawn(bar(HudNode::TroopsFill, TROOPS_BLUE));
                c.spawn(label(HudText::TroopsSub, 13.0, TEXT_DIM));
                c.spawn(label(HudText::Ratio, 14.0, TEXT));
                c.spawn(Node {
                    width: percent(100),
                    column_gap: px(3),
                    ..default()
                })
                .with_children(|row| {
                    for i in 0..9 {
                        row.spawn((
                            Button,
                            Node {
                                flex_grow: 1.0,
                                height: px(16),
                                border_radius: BorderRadius::all(px(3)),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                            HudBg::RatioSeg(i),
                            observe(move |_: On<Activate>, mut s: ResMut<ClientSettings>| {
                                s.attack_ratio = (i + 1) as f64 / 10.0;
                            }),
                            children![(Text::new(format!("{}", (i + 1) * 10)), font(10.0), TextColor(TEXT_DIM))],
                        ));
                    }
                });
            });
            // Gold
            col.spawn(card()).with_children(|c| {
                c.spawn(label(HudText::Gold, 17.0, ACCENT));
                c.spawn(label(HudText::GoldSub, 13.0, TEXT_DIM));
            });
            // Land
            col.spawn(card()).with_children(|c| {
                c.spawn(label(HudText::Land, 17.0, TEXT));
                c.spawn(bar(HudNode::LandFill, GOOD));
                c.spawn(label(HudText::LandSub, 13.0, TEXT_DIM));
            });
            // Attacks
            col.spawn((card(), HudNode::AttacksCard)).with_children(|c| {
                c.spawn(label(HudText::AttacksTitle, 13.0, TEXT_DIM));
                for i in 0..ATTACK_ROWS {
                    c.spawn((
                        Node {
                            width: percent(100),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        HudNode::AttackRow(i),
                    ))
                    .with_children(|row| {
                        row.spawn(label(HudText::AttackRow(i), 14.0, TEXT));
                        row.spawn((
                            Button,
                            Node {
                                padding: UiRect::axes(px(6), px(1)),
                                border_radius: BorderRadius::all(px(3)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 0.4, 0.35, 0.25)),
                            observe(
                                move |_: On<Activate>, ids: Res<AttackRowIds>, mut p: ResMut<PendingIntents>, mut n: ResMut<Notices>| {
                                    match ids.0.get(i) {
                                        Some(Some((id, true))) => {
                                            p.0.push(Intent::CancelBoat { player: HUMAN, unit: *id });
                                            n.push("Boat turning back (25% lost on landing at home)");
                                        }
                                        Some(Some((id, false))) => {
                                            p.0.push(Intent::CancelAttack { player: HUMAN, attack: *id });
                                            n.push("Retreating (25% losses against players)");
                                        }
                                        _ => {}
                                    }
                                },
                            ),
                            children![(Text::new("retreat"), font(11.0), TextColor(TEXT))],
                        ));
                    });
                }
            });
            // Leaderboard
            col.spawn(card()).with_children(|c| {
                c.spawn(label(HudText::LeaderTitle, 13.0, TEXT_DIM));
                for i in 0..LEADER_ROWS {
                    c.spawn((
                        Node {
                            width: percent(100),
                            column_gap: px(6),
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(px(3), px(1)),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        HudNode::LeaderRow(i),
                        HudBg::LeaderRow(i),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                width: px(10),
                                height: px(10),
                                border_radius: BorderRadius::all(px(2)),
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                            HudBg::LeaderSwatch(i),
                        ));
                        row.spawn(label(HudText::LeaderRow(i), 13.0, TEXT));
                    });
                }
            });
            // Notifications
            col.spawn(Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                ..default()
            })
            .with_children(|c| {
                for i in 0..NOTICE_ROWS {
                    c.spawn((
                        label(HudText::Notice(i), 14.0, TEXT),
                        Node { padding: UiRect::axes(px(6), px(2)), ..default() },
                        BackgroundColor(PANEL),
                        HudNode::NoticeRow(i),
                    ));
                }
            });
        });

    // ---- top-right: clock, pause, speed, help ----
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(8),
                right: px(8),
                column_gap: px(4),
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(8), px(5)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|bar| {
            bar.spawn((
                label(HudText::Clock, 16.0, TEXT),
                Node {
                    margin: UiRect::right(px(6)),
                    ..default()
                },
            ));
            bar.spawn((
                small_button("", HudBg::Pause),
                observe(|_: On<Activate>, mut s: ResMut<ClientSettings>| s.paused = !s.paused),
            ))
            .with_children(|b| {
                b.spawn(label(HudText::Pause, 14.0, TEXT));
            });
            for speed in 1..=3u32 {
                bar.spawn((
                    small_button(&format!("x{speed}"), HudBg::Speed(speed)),
                    observe(move |_: On<Activate>, mut s: ResMut<ClientSettings>| s.speed = speed),
                ));
            }
            bar.spawn((
                small_button("?", HudBg::Speed(0)),
                observe(|_: On<Activate>, mut s: ResMut<ClientSettings>| {
                    s.show_help = !s.show_help
                }),
            ));
        });

    // ---- spawn-phase / status banner (top centre) ----
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(10),
                left: percent(50),
                margin: UiRect::left(px(-230)),
                width: px(460),
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(12), px(6)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.75)),
            HudNode::PhaseBanner,
        ))
        .with_child((
            label(HudText::Phase, 18.0, ACCENT),
            TextLayout::justify(Justify::Center),
        ));

    // ---- hover info (bottom centre) ----
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(8),
                left: percent(50),
                margin: UiRect::left(px(-200)),
                width: px(400),
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(10), px(4)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(PANEL),
            HudNode::HoverPanel,
        ))
        .with_child(label(HudText::Hover, 14.0, TEXT));

    // ---- key help (H or "?") ----
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(52),
                right: px(8),
                width: px(330),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(10)),
                row_gap: px(3),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(PANEL_BORDER),
            HudNode::HelpCard,
        ))
        .with_children(|c| {
            c.spawn((Text::new("Controls"), font(16.0), TextColor(ACCENT)));
            for line in [
                "Left click      spawn / attack / boat across water",
                "Click own bldg  upgrade it",
                "Q E F G V B     city/port/factory/defense/silo/SAM",
                "N  Z X C        warship  atom/hydrogen/MIRV",
                "Right click     player: diplomacy; building: cancel",
                "1 - 9           attack size 10% - 90%",
                "R               retreat newest attack",
                "Space           pause",
                "F1 F2 F3        speed x1 x2 x3",
                "WASD / arrows   move camera",
                "Right drag      move camera",
                "Mouse wheel     zoom",
                "Minimap click   jump there",
                "H               show / hide this help",
                "Esc             cancel / close menu / skip tutorial",
            ] {
                c.spawn((Text::new(line), font(13.0), TextColor(TEXT)));
            }
        });

    // ---- attribution (licence requirement, always visible) ----
    commands.spawn((
        Text::new("laukofront - based on OpenFront. Copyright (c) OpenFront and Contributors. Not affiliated with OpenFront."),
        font(11.0),
        TextColor(Color::srgba(0.85, 0.87, 0.9, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(4),
            left: px(8),
            ..default()
        },
    ));
}

fn attack_row_text(l: &AttackLine) -> String {
    if l.outgoing && l.boat {
        format!("boat > {}  {}", l.other, fmt_num(l.troops))
    } else if l.outgoing {
        format!("> {}  {}", l.other, fmt_num(l.troops))
    } else {
        format!("< {}  {}", l.other, fmt_num(l.troops))
    }
}

fn phase_text(game: &crate::engine::Game, stats: &MatchStats) -> String {
    if stats.ended {
        return String::new();
    }
    if game.in_spawn_phase() {
        let cfg = &game.config().game;
        let left = cfg.spawn_phase_ticks.saturating_sub(game.ticks()) / cfg.tick_hz as u64;
        return format!("Choose your start: click unclaimed land ({left} s)");
    }
    match game.player(HUMAN) {
        Some(p) if !p.has_spawned() => "You did not spawn - watching the bots".into(),
        _ if game.is_spawn_immunity_active() => {
            "Spawn immunity: humans cannot attack each other yet".into()
        }
        _ => String::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn update_texts(
    game: Res<GameState>,
    settings: Res<ClientSettings>,
    notices: Res<Notices>,
    hovered: Res<Hovered>,
    stats: Res<MatchStats>,
    mut ids: ResMut<AttackRowIds>,
    mut texts: Query<(&HudText, &mut Text, &mut TextColor)>,
) {
    let g = &game.0;
    let h = hud_numbers(g);
    let lines = attack_lines(g);
    ids.0 = (0..ATTACK_ROWS)
        .map(|i| {
            lines
                .get(i)
                .filter(|l| l.outgoing)
                .map(|l| (l.attack, l.boat))
        })
        .collect();
    let rows = leaderboard(g, LEADER_ROWS - 1);
    let alive = g.alive_players().count();
    for (tag, mut text, mut color) in texts.iter_mut() {
        let (s, c): (String, Option<Color>) = match *tag {
            HudText::Troops => (
                h.as_ref().map_or(String::new(), |h| {
                    format!("Troops  {} / {}", fmt_num(h.troops), fmt_num(h.cap))
                }),
                None,
            ),
            HudText::TroopsSub => (
                h.as_ref().map_or(String::new(), |h| {
                    format!("{:+} per second", h.growth_per_sec.round() as i64)
                }),
                None,
            ),
            HudText::Ratio => (
                h.as_ref().map_or(String::new(), |h| {
                    format!(
                        "Attack size {:.0}%  =  {} troops",
                        settings.attack_ratio * 100.0,
                        fmt_num((h.troops * settings.attack_ratio).floor())
                    )
                }),
                None,
            ),
            HudText::Gold => (
                h.as_ref()
                    .map_or(String::new(), |h| format!("Gold  {}", fmt_num(h.gold))),
                None,
            ),
            HudText::GoldSub => (
                h.as_ref().map_or(String::new(), |h| {
                    let extra = (stats.gold_rate - h.gold_per_sec).max(0.0);
                    if extra >= 1.0 {
                        format!(
                            "+{} per second  ({} base, +{} trade & factories)",
                            fmt_num(stats.gold_rate),
                            fmt_num(h.gold_per_sec),
                            fmt_num(extra)
                        )
                    } else {
                        format!("+{} per second", fmt_num(h.gold_per_sec))
                    }
                }),
                None,
            ),
            HudText::Land => (
                h.as_ref().map_or(String::new(), |h| {
                    format!(
                        "Land  {:.1}%  ({} tiles)",
                        h.land_share * 100.0,
                        fmt_num(h.tiles as f64)
                    )
                }),
                None,
            ),
            HudText::LandSub => (
                h.as_ref().map_or(String::new(), |h| {
                    format!("Win at {:.0}% of the land", h.win_share * 100.0)
                }),
                None,
            ),
            HudText::AttacksTitle => (
                format!(
                    "ATTACKS  {} out / {} in",
                    lines.iter().filter(|l| l.outgoing).count(),
                    lines.iter().filter(|l| !l.outgoing).count()
                ),
                None,
            ),
            HudText::AttackRow(i) => match lines.get(i) {
                Some(l) => (
                    attack_row_text(l),
                    Some(if l.outgoing { TEXT } else { DANGER }),
                ),
                None => (String::new(), None),
            },
            HudText::LeaderTitle => (format!("LEADERBOARD  {alive} alive"), None),
            HudText::LeaderRow(i) => match rows.get(i) {
                Some(r) => (
                    format!(
                        "{:>2}. {:<9} {:>5.1}%  {:>7}",
                        r.rank,
                        match g.player(r.id).and_then(|p| p.team()) {
                            Some(t) => format!(
                                "T{} {}",
                                t + 1,
                                if r.is_me { "You" } else { r.name.as_str() }
                            ),
                            None => (if r.is_me { "You" } else { r.name.as_str() }).to_string(),
                        },
                        r.land_share * 100.0,
                        fmt_num(r.troops)
                    ),
                    // Allies green, traitors red.
                    Some(if r.is_me {
                        ACCENT
                    } else if g.is_allied(HUMAN, r.id) {
                        GOOD
                    } else if g.is_traitor(r.id) {
                        DANGER
                    } else {
                        TEXT
                    }),
                ),
                None => (String::new(), None),
            },
            HudText::Notice(i) => {
                let start = notices.0.len().saturating_sub(NOTICE_ROWS);
                match notices.0.get(start + i) {
                    Some(n) => (
                        n.text.clone(),
                        Some(match n.kind {
                            NoticeKind::Info => TEXT,
                            NoticeKind::Good => GOOD,
                            NoticeKind::Warning => ACCENT,
                            NoticeKind::Danger => DANGER,
                        }),
                    ),
                    None => (String::new(), None),
                }
            }
            HudText::Clock => (clock(g), None),
            HudText::Pause => ((if settings.paused { ">" } else { "II" }).to_string(), None),
            HudText::Phase => (phase_text(g, &stats), None),
            HudText::Hover => (hover_text(g, hovered.0), None),
        };
        if text.0 != s {
            text.0 = s;
        }
        if let Some(c) = c {
            if color.0 != c {
                color.0 = c;
            }
        }
    }
}

fn hover_text(g: &crate::engine::Game, hovered: Option<u32>) -> String {
    let Some(t) = hovered else {
        return String::new();
    };
    let map = g.map();
    let terrain = format!("{:?}", map.terrain_type(t));
    let building = g
        .units()
        .iter()
        .filter(|u| {
            u.is_active() && u.kind().is_structure() && map.manhattan_dist(u.tile(), t) <= 2
        })
        .min_by_key(|u| map.manhattan_dist(u.tile(), t))
        .map(|u| {
            if u.under_construction(g.ticks()) {
                format!(
                    "  |  {} (building {:.0}%)",
                    u.kind().name(),
                    u.construction_progress(g.ticks()) * 100.0
                )
            } else {
                format!("  |  {} level {}", u.kind().name(), u.level())
            }
        })
        .unwrap_or_default();
    let text = match g.owner(t).and_then(|o| g.player(o)) {
        Some(p) => format!(
            "{terrain} - {}  {} tiles, {} troops",
            if p.id() == HUMAN { "You" } else { p.name() },
            fmt_num(p.num_tiles() as f64),
            fmt_num(p.troops() as f64)
        ),
        None if map.is_passable_land(t) => format!("{terrain} - unclaimed"),
        None => terrain,
    };
    text + &building
}

fn update_nodes(
    game: Res<GameState>,
    notice_count: Res<Notices>,
    settings: Res<ClientSettings>,
    stats: Res<MatchStats>,
    hovered: Res<Hovered>,
    mut nodes: Query<(&HudNode, &mut Node)>,
) {
    let g = &game.0;
    let h = hud_numbers(g);
    let lines = attack_lines(g).len();
    let rows = leaderboard(g, LEADER_ROWS - 1).len();
    let phase = !phase_text(g, &stats).is_empty();
    let notices = notice_count.0.len().min(NOTICE_ROWS);
    let show = |on: bool| if on { Display::Flex } else { Display::None };
    for (tag, mut node) in nodes.iter_mut() {
        match *tag {
            HudNode::TroopsFill => {
                let w = h
                    .as_ref()
                    .map_or(0.0, |h| (h.troops / h.cap.max(1.0)).clamp(0.0, 1.0));
                set_width(&mut node, w);
            }
            HudNode::LandFill => {
                let w = h
                    .as_ref()
                    .map_or(0.0, |h| (h.land_share / h.win_share).clamp(0.0, 1.0));
                set_width(&mut node, w);
            }
            HudNode::AttackRow(i) => set_display(&mut node, show(i < lines)),
            HudNode::AttacksCard => set_display(&mut node, show(lines > 0)),
            HudNode::LeaderRow(i) => set_display(&mut node, show(i < rows)),
            HudNode::PhaseBanner => set_display(&mut node, show(phase)),
            HudNode::HelpCard => set_display(&mut node, show(settings.show_help)),
            HudNode::HoverPanel => set_display(&mut node, show(hovered.0.is_some())),
            HudNode::NoticeRow(i) => set_display(&mut node, show(i < notices)),
        }
    }
}

fn set_width(node: &mut Node, share: f64) {
    let w = percent((share * 100.0) as f32);
    if node.width != w {
        node.width = w;
    }
}

fn set_display(node: &mut Node, d: Display) {
    if node.display != d {
        node.display = d;
    }
}

fn update_backgrounds(
    game: Res<GameState>,
    settings: Res<ClientSettings>,
    mut bgs: Query<(&HudBg, &mut BackgroundColor)>,
) {
    let rows = leaderboard(&game.0, LEADER_ROWS - 1);
    let selected = (settings.attack_ratio * 10.0).round() as usize;
    for (tag, mut bg) in bgs.iter_mut() {
        let c = match *tag {
            HudBg::RatioSeg(i) => {
                if i < selected {
                    TROOPS_BLUE.with_alpha(0.35 + 0.65 * (i + 1) as f32 / selected.max(1) as f32)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 0.08)
                }
            }
            HudBg::Speed(s) => {
                if s == settings.speed || (s == 0 && settings.show_help) {
                    ACCENT.with_alpha(0.45)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 0.08)
                }
            }
            HudBg::Pause => {
                if settings.paused {
                    DANGER.with_alpha(0.5)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 0.08)
                }
            }
            HudBg::LeaderSwatch(i) => rows.get(i).map_or(Color::NONE, |r| {
                let c = palette::player_color(r.id);
                Color::srgb_u8(c[0], c[1], c[2])
            }),
            HudBg::LeaderRow(i) => {
                if rows.get(i).is_some_and(|r| r.is_me) {
                    ACCENT.with_alpha(0.16)
                } else {
                    Color::NONE
                }
            }
        };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}
