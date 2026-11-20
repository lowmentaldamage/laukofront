//! Build bar (bottom centre): one button per building with its key and current cost. Click (or
//! press the key) to choose, then click your land to build; right click or Esc cancels.
//! Unaffordable buildings are dimmed.

use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

use super::{fmt_num, font, ACCENT, PANEL, TEXT, TEXT_DIM};
use crate::client::{ClientSettings, GameState};
use crate::engine::structures::build_cost;
use crate::engine::units::UnitKind;
use crate::engine::HUMAN;

pub const KEYS: [(&str, UnitKind); 4] = [
    ("Q", UnitKind::City),
    ("E", UnitKind::Port),
    ("F", UnitKind::Factory),
    ("G", UnitKind::DefensePost),
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

/// What each building does (shown under the bar while it is selected).
pub fn describe(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::City => "City: +250K max troops per level.",
        UnitKind::Port => {
            "Port: must touch the ocean. Trade ships earn gold for you and the other port."
        }
        UnitKind::Factory => {
            "Factory: trains to your cities and ports within 110 tiles earn gold every 10 s."
        }
        UnitKind::DefensePost => {
            "Defense post: attacks within 30 tiles lose 5x troops and move 3x slower."
        }
        _ => "",
    }
}

fn setup(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(40),
            left: percent(50),
            margin: UiRect::left(px(-250)),
            width: px(500),
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
                    column_gap: px(6),
                    padding: UiRect::all(px(6)),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(PANEL),
            ))
            .with_children(|bar| {
                for (key, kind) in KEYS {
                    bar.spawn((
                        Button,
                        Node {
                            width: px(112),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(px(6), px(4)),
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
                        b.spawn((
                            Text::new(format!("{}  [{key}]", kind.name())),
                            font(13.0),
                            TextColor(TEXT),
                        ));
                        b.spawn((
                            Text::new(""),
                            font(12.0),
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
        let afford = gold >= build_cost(g, HUMAN, b.0);
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
        let cost = build_cost(g, HUMAN, c.0);
        let s = format!("{} gold", fmt_num(cost as f64));
        if text.0 != s {
            text.0 = s;
        }
        color.0 = if gold >= cost { ACCENT } else { TEXT_DIM };
    }
    for (mut text, mut bg) in hint.iter_mut() {
        let s = match settings.build {
            Some(k) => format!(
                "{}\nClick your land to build. Right click or Esc to cancel.",
                describe(k)
            ),
            None => String::new(),
        };
        bg.0 = if s.is_empty() { Color::NONE } else { PANEL };
        if text.0 != s {
            text.0 = s;
        }
    }
}
