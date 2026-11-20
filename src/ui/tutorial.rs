//! On-map tutorial: one hint bubble at a time, placed next to your land, that moves on when you
//! do what it says. Esc skips it; finishing or skipping is remembered in the settings file.

use bevy::prelude::*;

use super::labels::Labels;
use super::{font, ACCENT, TEXT, TEXT_DIM};
use crate::client::{ClientSettings, GameState, SavedSettings};
use crate::engine::HUMAN;
use crate::input::PlayerActions;
use crate::render::camera::MapCamera;

/// Tutorial steps, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Spawn,
    Expand,
    Size,
    AttackPlayer,
    Done,
}

impl Step {
    pub fn text(self) -> &'static str {
        match self {
            Step::Spawn => "Click any unclaimed land to place your start.\nYou have 10 seconds.",
            Step::Expand => "Your land has a gold border.\nClick the free land right next to it to expand.",
            Step::Size => "Press 1-9, or click the bar on the left,\nto choose how many troops an attack sends.",
            Step::AttackPlayer => {
                "When you touch another player, click their land\nto attack them. R pulls your newest attack back."
            }
            Step::Done => "",
        }
    }

    pub fn number(self) -> usize {
        match self {
            Step::Spawn => 1,
            Step::Expand => 2,
            Step::Size => 3,
            Step::AttackPlayer => 4,
            Step::Done => 5,
        }
    }
}

/// Next step given what the player has done so far. Pure so it can be tested.
pub fn advance(step: Step, actions: &PlayerActions, spawned: bool) -> Step {
    match step {
        Step::Spawn if spawned => Step::Expand,
        Step::Expand if actions.attacked_land => Step::Size,
        Step::Size if actions.changed_size => Step::AttackPlayer,
        Step::AttackPlayer if actions.attacked_player => Step::Done,
        s => s,
    }
}

#[derive(Resource)]
pub struct Tutorial {
    pub step: Step,
}

#[derive(Component)]
struct Bubble;

#[derive(Component)]
struct BubbleText;

pub struct TutorialPlugin;

impl Plugin for TutorialPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, (progress, place_bubble).chain());
    }
}

fn setup(mut commands: Commands, saved: Res<SavedSettings>) {
    let step = if saved.tutorial_done {
        Step::Done
    } else {
        Step::Spawn
    };
    commands.insert_resource(Tutorial { step });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                max_width: px(480),
                padding: UiRect::axes(px(12), px(8)),
                row_gap: px(4),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(8)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.07, 0.1)),
            BorderColor::all(ACCENT),
            Bubble,
        ))
        .with_children(|b| {
            b.spawn((Text::new(""), font(15.0), TextColor(TEXT), BubbleText));
            b.spawn((
                Text::new("Esc: skip tutorial    H: all controls"),
                font(11.0),
                TextColor(TEXT_DIM),
            ));
        });
}

fn progress(
    keys: Res<ButtonInput<KeyCode>>,
    game: Res<GameState>,
    actions: Res<PlayerActions>,
    mut tutorial: ResMut<Tutorial>,
    mut saved: ResMut<SavedSettings>,
    mut settings: ResMut<ClientSettings>,
) {
    if tutorial.step == Step::Done {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        tutorial.step = Step::Done;
    } else {
        let spawned = game.0.player(HUMAN).is_some_and(|p| p.has_spawned());
        let next = advance(tutorial.step, &actions, spawned);
        if next != tutorial.step {
            tutorial.step = next;
        }
    }
    if tutorial.step == Step::Done {
        saved.tutorial_done = true;
        saved.save();
        settings.show_help = false;
    }
}

fn place_bubble(
    tutorial: Res<Tutorial>,
    labels: Res<Labels>,
    camera: Single<(&Camera, &GlobalTransform), With<MapCamera>>,
    window: Single<&Window>,
    mut bubbles: Query<&mut Node, With<Bubble>>,
    mut texts: Query<&mut Text, With<BubbleText>>,
) {
    let (cam, cam_t) = *camera;
    let show = tutorial.step != Step::Done;
    // Next to your land if you have some on screen, otherwise near the screen centre.
    let anchor = labels
        .human_center()
        .and_then(|c| cam.world_to_viewport(cam_t, c.extend(0.0)).ok())
        .filter(|p| p.x > 0.0 && p.y > 0.0 && p.x < window.width() && p.y < window.height())
        .map(|p| Vec2::new(p.x + 40.0, p.y + 30.0))
        .unwrap_or(Vec2::new(
            window.width() * 0.5 - 150.0,
            window.height() * 0.35,
        ));
    let x = anchor.x.clamp(320.0, (window.width() - 400.0).max(320.0));
    let y = anchor.y.clamp(60.0, (window.height() - 160.0).max(60.0));
    for mut n in bubbles.iter_mut() {
        n.display = if show { Display::Flex } else { Display::None };
        n.left = px(x);
        n.top = px(y);
    }
    for mut t in texts.iter_mut() {
        let s = format!(
            "{}/4  {}",
            tutorial.step.number().min(4),
            tutorial.step.text()
        );
        if show && t.0 != s {
            t.0 = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_advance_only_when_done_in_order() {
        let mut a = PlayerActions::default();
        assert_eq!(advance(Step::Spawn, &a, false), Step::Spawn);
        assert_eq!(advance(Step::Spawn, &a, true), Step::Expand);
        a.changed_size = true; // doing a later step first does not skip ahead
        assert_eq!(advance(Step::Expand, &a, true), Step::Expand);
        a.attacked_land = true;
        assert_eq!(advance(Step::Expand, &a, true), Step::Size);
        assert_eq!(advance(Step::Size, &a, true), Step::AttackPlayer);
        a.attacked_player = true;
        assert_eq!(advance(Step::AttackPlayer, &a, true), Step::Done);
        assert_eq!(advance(Step::Done, &a, true), Step::Done);
    }
}
