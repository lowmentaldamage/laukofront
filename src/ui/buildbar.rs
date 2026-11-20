//! Build bar (bottom centre): one button per building, warship and missile with its key and
//! current price. Click (or press the key) to choose, then click the map; right click or Esc
//! cancels. Things you cannot afford are dimmed.

use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

use super::{fmt_num, font, ACCENT, PANEL, TEXT, TEXT_DIM};
use crate::client::{ClientSettings, GameState};
use crate::engine::structures::build_cost;
use crate::engine::units::UnitKind;
use crate::engine::weapons::nuke_cost;
use crate::engine::{Game, HUMAN};

/// Buildings: key and kind.
pub const KEYS: [(&str, UnitKind); 6] = [
    ("Q", UnitKind::City),
    ("E", UnitKind::Port),
    ("F", UnitKind::Factory),
    ("G", UnitKind::DefensePost),
    ("V", UnitKind::MissileSilo),
    ("B", UnitKind::SamLauncher),
];

/// Warship and missiles: key and kind.
pub const MILITARY: [(&str, UnitKind); 4] = [
    ("N", UnitKind::Warship),
    ("Z", UnitKind::AtomBomb),
    ("X", UnitKind::HydrogenBomb),
    ("C", UnitKind::Mirv),
];

#[derive(Component)]
struct BuildButton(UnitKind);

#[derive(Component)]
struct BuildCost(UnitKind);

#[derive(Component)]
struct BuildHint;

pub struct BuildBarPlugin;

impl Plugin for BuildBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, update);
    }
}

/// Price of anything in the bar.
pub fn cost_of(game: &Game, kind: UnitKind) -> i64 {
    if kind.is_nuke() {
        nuke_cost(game, kind)
    } else {
        build_cost(game, HUMAN, kind)
    }
}

fn short_name(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::City => "City",
        UnitKind::Port => "Port",
        UnitKind::Factory => "Factory",
        UnitKind::DefensePost => "Defense",
        UnitKind::MissileSilo => "Silo",
        UnitKind::SamLauncher => "SAM",
        UnitKind::Warship => "Warship",
        UnitKind::AtomBomb => "Atom",
        UnitKind::HydrogenBomb => "Hydrogen",
        UnitKind::Mirv => "MIRV",
        _ => "",
    }
}

/// What each choice does (shown above the bar while it is selected).
pub fn describe(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::City => "City: +250K max troops per level.",
        UnitKind::Port => "Port: must touch the ocean. Trade ships earn gold for you and the other port.",
        UnitKind::Factory => "Factory: trains to your cities and ports within 110 tiles earn gold every 10 s.",
        UnitKind::DefensePost => "Defense post: attacks within 30 tiles lose 5x troops and move 3x slower.",
        UnitKind::MissileSilo => "Missile silo: launches nukes, one at a time per level (9 s reload).",
        UnitKind::SamLauncher => "SAM launcher: shoots down atom and hydrogen bombs in its range (circle).",
        UnitKind::Warship => "Warship: click water near your port. Sinks boats, fights warships, captures trade ships.",
        UnitKind::AtomBomb => "Atom bomb: destroys a radius of 12-30 tiles. Click any tile. Needs a silo.",
        UnitKind::HydrogenBomb => "Hydrogen bomb: destroys a radius of 80-100 tiles. Needs a silo.",
        UnitKind::Mirv => "MIRV: 350 warheads over the target player's land. SAMs cannot stop it.",
        _ => "",
    }
}

fn setup(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(40),
            left: percent(50),
            margin: UiRect::left(px(-340)),
            width: px(680),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new(""),
                font(13.0),
                TextColor(ACCENT),
                TextLayout::justify(Justify::Center),
                Node {
                    padding: UiRect::axes(px(8), px(3)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BuildHint,
            ));
            col.spawn((
                Node {
                    column_gap: px(4),
                    padding: UiRect::all(px(5)),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(PANEL),
            ))
            .with_children(|bar| {
                for (i, (key, kind)) in KEYS.into_iter().chain(MILITARY).enumerate() {
                    if i == KEYS.len() {
                        // Gap between buildings and weapons.
                        bar.spawn(Node {
                            width: px(8),
                            ..default()
                        });
                    }
                    bar.spawn((
                        Button,
                        Node {
                            width: px(62),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(px(3), px(3)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(6)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.06)),
                        BorderColor::all(Color::NONE),
                        BuildButton(kind),
                        observe(move |_: On<Activate>, mut s: ResMut<ClientSettings>| {
                            s.build = if s.build == Some(kind) {
                                None
                            } else {
                                Some(kind)
                            };
                        }),
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new(short_name(kind)), font(11.0), TextColor(TEXT)));
                        b.spawn((
                            Text::new(format!("[{key}]")),
                            font(10.0),
                            TextColor(TEXT_DIM),
                        ));
                        b.spawn((
                            Text::new(""),
                            font(11.0),
                            TextColor(ACCENT),
                            BuildCost(kind),
                        ));
                    });
                }
            });
        });
}

#[allow(clippy::type_complexity)]
fn update(
    game: Res<GameState>,
    settings: Res<ClientSettings>,
    mut buttons: Query<(&BuildButton, &mut BackgroundColor, &mut BorderColor)>,
    mut costs: Query<(&BuildCost, &mut Text, &mut TextColor), Without<BuildHint>>,
    mut hint: Query<(&mut Text, &mut BackgroundColor), (With<BuildHint>, Without<BuildButton>)>,
) {
    let g = &game.0;
    let gold = g.player(HUMAN).map_or(0, |p| p.gold());
    for (b, mut bg, mut border) in buttons.iter_mut() {
        let selected = settings.build == Some(b.0);
        let afford = gold >= cost_of(g, b.0);
        bg.0 = if selected {
            ACCENT.with_alpha(0.3)
        } else if afford {
            Color::srgba(1.0, 1.0, 1.0, 0.1)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.03)
        };
        *border = BorderColor::all(if selected { ACCENT } else { Color::NONE });
    }
    for (c, mut text, mut color) in costs.iter_mut() {
        let cost = cost_of(g, c.0);
        let s = fmt_num(cost as f64);
        if text.0 != s {
            text.0 = s;
        }
        color.0 = if gold >= cost { ACCENT } else { TEXT_DIM };
    }
    for (mut text, mut bg) in hint.iter_mut() {
        let s = match settings.build {
            Some(k) => format!(
                "{}\nClick the map. Right click or Esc to cancel.",
                describe(k)
            ),
            None if settings.selected_ship.is_some() => {
                "Warship selected: click water to send it there. Esc cancels.".to_string()
            }
            None => String::new(),
        };
        bg.0 = if s.is_empty() { Color::NONE } else { PANEL };
        if text.0 != s {
            text.0 = s;
        }
    }
}
