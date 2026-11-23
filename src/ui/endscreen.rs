//! End screen: shown when someone wins or you are eliminated. Restart (next seed), keep watching,
//! or quit.

use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

use super::{clock, font, ACCENT, DANGER, GOOD, PANEL, TEXT, TEXT_DIM};
use crate::client::{GameState, MatchStats, RestartRequested, Screen};
use crate::engine::{Game, HUMAN};

#[derive(Component)]
struct EndPanel;

#[derive(Component)]
struct EndTitle;

#[derive(Component)]
struct EndBody;

const CHART_BARS: usize = 40;

#[derive(Component)]
struct EndBar(usize);

/// Bar heights (0..1, relative to the best moment) for the land chart. Pure for tests.
pub fn chart_bars(history: &[f32], bars: usize) -> Vec<f32> {
    if history.is_empty() {
        return vec![0.0; bars];
    }
    let max = history.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
    (0..bars)
        .map(|i| history[(i * history.len() / bars).min(history.len() - 1)] / max)
        .collect()
}

/// The player closed the panel to keep watching.
#[derive(Resource, Default)]
struct Dismissed(bool);

pub struct EndScreenPlugin;

impl Plugin for EndScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dismissed>()
            .add_systems(Startup, setup)
            .add_systems(Update, update);
        // A new match brings the panel back.
        app.add_systems(
            Update,
            |stats: Res<MatchStats>, mut d: ResMut<Dismissed>| {
                if stats.is_changed() && !stats.ended && d.0 {
                    d.0 = false;
                }
            },
        );
    }
}

/// Title, body and colour of the end screen. Pure for tests.
pub fn end_text(game: &Game, stats: &MatchStats) -> (String, String, Color) {
    let me = game.player(HUMAN);
    let peak = format!(
        "Your best: {:.1}% of the land, {} troops, {} buildings",
        stats.peak_land * 100.0,
        super::fmt_num(stats.peak_troops as f64),
        stats.peak_buildings
    );
    let time = format!("Match time {}", clock(game));
    if let Some(t) = game.winner_team() {
        let mine = me.and_then(|p| p.team());
        return if mine == Some(t) {
            (
                "VICTORY".into(),
                format!(
                    "Your team (team {}) owns most of the land.
{time}
{peak}",
                    t + 1
                ),
                GOOD,
            )
        } else {
            (
                "DEFEAT".into(),
                format!(
                    "Team {} won the match.
{time}
{peak}",
                    t + 1
                ),
                DANGER,
            )
        };
    }
    match game.winner() {
        Some(w) if w == HUMAN => (
            "VICTORY".into(),
            format!("You own most of the land.\n{time}\n{peak}"),
            GOOD,
        ),
        Some(w) => {
            let name = game.player(w).map_or("?", |p| p.name());
            (
                "DEFEAT".into(),
                format!("{name} won the match.\n{time}\n{peak}"),
                DANGER,
            )
        }
        None if me.is_some_and(|p| p.has_spawned() && !p.is_alive()) => (
            "ELIMINATED".into(),
            format!("Your last land was taken.\n{time}\n{peak}"),
            DANGER,
        ),
        None => (String::new(), String::new(), TEXT),
    }
}

fn button(text: &str) -> impl Bundle {
    (
        Button,
        Node {
            padding: UiRect::axes(px(18), px(8)),
            border_radius: BorderRadius::all(px(6)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)),
        children![(Text::new(text), font(16.0), TextColor(TEXT))],
    )
}

fn setup(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: percent(30),
                margin: UiRect::left(px(-210)),
                width: px(420),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(px(20)),
                row_gap: px(10),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(10)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(ACCENT),
            EndPanel,
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), font(34.0), TextColor(ACCENT), EndTitle));
            p.spawn((
                Text::new(""),
                font(16.0),
                TextColor(TEXT),
                TextLayout::justify(Justify::Center),
                EndBody,
            ));
            // Your land over the match.
            p.spawn((
                Node {
                    width: px(360),
                    height: px(60),
                    align_items: AlignItems::FlexEnd,
                    column_gap: px(1),
                    padding: UiRect::all(px(3)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
            ))
            .with_children(|chart| {
                for i in 0..CHART_BARS {
                    chart.spawn((
                        Node {
                            flex_grow: 1.0,
                            height: percent(0),
                            ..default()
                        },
                        BackgroundColor(ACCENT.with_alpha(0.8)),
                        EndBar(i),
                    ));
                }
            });
            p.spawn((
                Text::new("Your land over the match"),
                font(11.0),
                TextColor(TEXT_DIM),
            ));
            p.spawn(Node {
                column_gap: px(10),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    button("New match"),
                    observe(
                        |_: On<Activate>,
                         mut r: ResMut<RestartRequested>,
                         mut d: ResMut<Dismissed>| {
                            r.0 = true;
                            d.0 = false;
                        },
                    ),
                ));
                row.spawn((
                    button("Main menu"),
                    observe(
                        |_: On<Activate>, mut s: ResMut<Screen>, mut d: ResMut<Dismissed>| {
                            *s = Screen::MainMenu;
                            d.0 = true;
                        },
                    ),
                ));
                row.spawn((
                    button("Keep watching"),
                    observe(|_: On<Activate>, mut d: ResMut<Dismissed>| d.0 = true),
                ));
                row.spawn((
                    button("Quit"),
                    observe(|_: On<Activate>, mut exit: MessageWriter<AppExit>| {
                        exit.write(AppExit::Success);
                    }),
                ));
            });
            p.spawn((
                Text::new("laukofront - based on OpenFront"),
                font(11.0),
                TextColor(TEXT_DIM),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn update(
    game: Res<GameState>,
    stats: Res<MatchStats>,
    dismissed: Res<Dismissed>,
    mut panel: Query<&mut Node, With<EndPanel>>,
    mut title: Query<(&mut Text, &mut TextColor), (With<EndTitle>, Without<EndBody>)>,
    mut body: Query<&mut Text, (With<EndBody>, Without<EndTitle>)>,
    mut bars: Query<(&EndBar, &mut Node), Without<EndPanel>>,
) {
    let show = stats.ended && !dismissed.0;
    for mut n in panel.iter_mut() {
        let d = if show { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    if !show {
        return;
    }
    let heights = chart_bars(&stats.land_history, CHART_BARS);
    for (bar, mut n) in bars.iter_mut() {
        let h = percent(heights[bar.0] * 100.0);
        if n.height != h {
            n.height = h;
        }
    }
    let (t, b, c) = end_text(&game.0, &stats);
    for (mut text, mut color) in title.iter_mut() {
        if text.0 != t {
            text.0 = t.clone();
            color.0 = c;
        }
    }
    for mut text in body.iter_mut() {
        if text.0 != b {
            text.0 = b.clone();
        }
    }
}
