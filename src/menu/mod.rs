//! Menus (build-guide Stage 12, port milestone P12): main menu, match setup, settings, credits and
//! the pause menu. They are drawn over the live match (a bots-only match plays behind the main
//! menu). Each screen is rebuilt when `Screen` changes; values update every frame.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode};

use crate::client::save::{has_save, LoadRequested, SaveRequested};
use crate::client::{ClientSettings, CurrentSetup, SavedSettings, Screen, StartMatch};
use crate::config::{Difficulty, MapSize};
use crate::engine::{MapChoice, MatchSetup};
use crate::ui::{font, ACCENT, PANEL, PANEL_BORDER, TEXT, TEXT_DIM};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SetupDraft>()
            .init_resource::<SettingsBack>()
            .add_systems(Startup, apply_settings_now)
            .add_systems(
                Update,
                (
                    rebuild_menu,
                    update_values,
                    apply_settings,
                    pause_state,
                    hide_game_ui,
                ),
            );
    }
}

/// The match being set up in the setup screen.
#[derive(Resource, Debug, Clone, Copy)]
pub struct SetupDraft(pub MatchSetup);

impl Default for SetupDraft {
    fn default() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| (d.as_nanos() % 1_000_000_007) as i64)
            .unwrap_or(1);
        let mut s = MatchSetup::bots_only(MapSize::Medium, seed, 20);
        s.nations = MatchSetup::default_nations(MapSize::Medium);
        Self(s)
    }
}

/// Where the settings screen returns to.
#[derive(Resource, Default)]
struct SettingsBack(Screen);

#[derive(Component)]
struct MenuRoot;

/// A value shown next to a setting; refreshed every frame.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Map,
    MapInfo,
    Size,
    Bots,
    Nations,
    Difficulty,
    Teams,
    Seed,
    Volume,
    Fullscreen,
    Vsync,
    UiScale,
    AttackSize,
    Tutorial,
}

/// Everything a menu button may change.
#[derive(SystemParam)]
struct MenuCtx<'w> {
    screen: ResMut<'w, Screen>,
    back: ResMut<'w, SettingsBack>,
    draft: ResMut<'w, SetupDraft>,
    saved: ResMut<'w, SavedSettings>,
    start: ResMut<'w, StartMatch>,
    settings: ResMut<'w, ClientSettings>,
    current: Res<'w, CurrentSetup>,
    save: ResMut<'w, SaveRequested>,
    load: ResMut<'w, LoadRequested>,
    exit: MessageWriter<'w, AppExit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Go(Screen),
    OpenSettings,
    SettingsDone,
    Start,
    Resume,
    Restart,
    Save,
    Load,
    Quit,
    Step(Field, i32),
    NewSeed,
    UseSuggested,
}

fn steps<T: Copy + PartialEq>(list: &[T], cur: T, d: i32) -> T {
    let i = list.iter().position(|&x| x == cur).unwrap_or(0) as i32;
    let n = list.len() as i32;
    list[((i + d).rem_euclid(n)) as usize]
}

fn act(a: Action, c: &mut MenuCtx) {
    match a {
        Action::Go(s) => *c.screen = s,
        Action::OpenSettings => {
            c.back.0 = *c.screen;
            *c.screen = Screen::Settings;
        }
        Action::SettingsDone => {
            c.saved.save();
            *c.screen = c.back.0;
        }
        Action::Start => {
            c.start.0 = Some(c.draft.0);
            c.settings.attack_ratio = c.saved.default_attack_ratio;
            c.settings.speed = 1;
            *c.screen = Screen::Playing;
        }
        Action::Resume => *c.screen = Screen::Playing,
        Action::Restart => {
            c.start.0 = Some(c.current.0);
            *c.screen = Screen::Playing;
        }
        Action::Save => {
            c.save.0 = true;
            *c.screen = Screen::Playing;
        }
        Action::Load => {
            c.load.0 = true;
            *c.screen = Screen::Playing;
        }
        Action::Quit => {
            c.exit.write(AppExit::Success);
        }
        Action::NewSeed => {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| (d.as_nanos() % 1_000_000_007) as i64)
                .unwrap_or(1);
            c.draft.0.seed = t;
        }
        Action::UseSuggested => {
            let d = &mut c.draft.0;
            match d.map {
                MapChoice::Earth(e) => (d.bots, d.nations) = e.suggested_players(),
                MapChoice::Generated(_) => {
                    d.bots = 20;
                    d.nations = MatchSetup::default_nations(d.size);
                }
            }
        }
        Action::Step(f, dir) => step(f, dir, c),
    }
}

fn step(f: Field, dir: i32, c: &mut MenuCtx) {
    let d = &mut c.draft.0;
    let s = &mut *c.saved;
    match f {
        Field::Map => {
            d.map = steps(&MapChoice::all(), d.map, dir);
            if let MapChoice::Earth(e) = d.map {
                (d.bots, d.nations) = e.suggested_players();
            }
        }
        Field::Size => d.size = steps(&MapSize::ALL, d.size, dir),
        Field::Bots => d.bots = (d.bots as i32 + dir * 5).clamp(0, 100) as u32,
        Field::Nations => d.nations = (d.nations as i32 + dir).clamp(0, 30) as u32,
        Field::Difficulty => d.difficulty = steps(&Difficulty::ALL, d.difficulty, dir),
        Field::Teams => d.teams = steps(&[0u8, 2, 3, 4], d.teams, dir),
        Field::Seed => d.seed = d.seed.wrapping_add(dir as i64),
        Field::Volume => {
            s.volume = ((s.volume * 10.0).round() + dir as f32).clamp(0.0, 10.0) / 10.0
        }
        Field::Fullscreen => s.fullscreen = !s.fullscreen,
        Field::Vsync => s.vsync = !s.vsync,
        Field::UiScale => {
            s.ui_scale = ((s.ui_scale * 20.0).round() + dir as f32).clamp(15.0, 30.0) / 20.0
        }
        Field::AttackSize => {
            s.default_attack_ratio =
                ((s.default_attack_ratio * 10.0).round() + dir as f64).clamp(1.0, 9.0) / 10.0
        }
        Field::Tutorial => s.tutorial_done = !s.tutorial_done,
        Field::MapInfo => {}
    }
}

fn map_title(m: MapChoice) -> String {
    match m {
        MapChoice::Generated(s) => format!("Generated: {}", s.name()),
        MapChoice::Earth(e) => format!("Earth: {}", e.title()),
    }
}

/// The text of a value. Pure, for tests.
fn field_text(f: Field, d: &MatchSetup, s: &SavedSettings) -> String {
    let on = |b: bool| if b { "on" } else { "off" }.to_string();
    match f {
        Field::Map => map_title(d.map),
        Field::MapInfo => match d.map {
            MapChoice::Generated(_) => "A new map from the seed. Rivers, peaks and biomes.".into(),
            MapChoice::Earth(e) => {
                let r = e.region();
                let (b, n) = e.suggested_players();
                format!(
                    "{}x{} tiles. Suggested: {b} bots, {n} nations.",
                    r.width, r.height
                )
            }
        },
        Field::Size => match d.map {
            MapChoice::Generated(_) => format!("{:?}", d.size),
            MapChoice::Earth(_) => "fixed".into(),
        },
        Field::Bots => d.bots.to_string(),
        Field::Nations => d.nations.to_string(),
        Field::Difficulty => d.difficulty.name().into(),
        Field::Teams => match d.teams {
            0 => "free for all".into(),
            t => format!("{t} teams"),
        },
        Field::Seed => d.seed.to_string(),
        Field::Volume => format!("{:.0}%", s.volume * 100.0),
        Field::Fullscreen => on(s.fullscreen),
        Field::Vsync => on(s.vsync),
        Field::UiScale => format!("{:.0}%", s.ui_scale * 100.0),
        Field::AttackSize => format!("{:.0}%", s.default_attack_ratio * 100.0),
        Field::Tutorial => on(!s.tutorial_done),
    }
}

fn button(label: &str, a: Action, wide: bool) -> impl Bundle {
    (
        Button,
        Node {
            width: if wide { px(260) } else { Val::Auto },
            min_width: px(34),
            padding: UiRect::axes(px(12), px(7)),
            justify_content: JustifyContent::Center,
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
        BorderColor::all(PANEL_BORDER),
        observe(move |_: On<Activate>, mut c: MenuCtx| act(a, &mut c)),
        children![(Text::new(label.to_string()), font(16.0), TextColor(TEXT))],
    )
}

/// A row: label, < value >.
fn value_row(p: &mut ChildSpawnerCommands, label: &str, f: Field) {
    p.spawn(Node {
        width: px(520),
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|r| {
        r.spawn((
            Text::new(label.to_string()),
            font(16.0),
            TextColor(TEXT_DIM),
        ));
        r.spawn(Node {
            column_gap: px(8),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|v| {
            v.spawn(button("<", Action::Step(f, -1), false));
            v.spawn((
                Text::new(""),
                font(16.0),
                TextColor(ACCENT),
                TextLayout::justify(Justify::Center),
                Node {
                    width: px(220),
                    ..default()
                },
                f,
            ));
            v.spawn(button(">", Action::Step(f, 1), false));
        });
    });
}

fn title(p: &mut ChildSpawnerCommands, text: &str, size: f32) {
    p.spawn((Text::new(text.to_string()), font(size), TextColor(ACCENT)));
}

fn rebuild_menu(
    mut commands: Commands,
    screen: Res<Screen>,
    roots: Query<Entity, With<MenuRoot>>,
    mut built: Local<Option<Screen>>,
) {
    if *built == Some(*screen) {
        return;
    }
    *built = Some(*screen);
    for e in roots.iter() {
        commands.entity(e).despawn();
    }
    if *screen == Screen::Playing {
        return;
    }
    let shade = if *screen == Screen::Paused {
        0.45
    } else {
        0.62
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, shade)),
            GlobalZIndex(50),
            MenuRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(10),
                    padding: UiRect::axes(px(36), px(28)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(10)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(PANEL_BORDER),
            ))
            .with_children(|p| match *screen {
                Screen::MainMenu => {
                    title(p, "laukofront", 48.0);
                    p.spawn((
                        Text::new("Conquer the map: expand, ally, build, sail and nuke."),
                        font(15.0),
                        TextColor(TEXT_DIM),
                    ));
                    p.spawn(Node {
                        height: px(10),
                        ..default()
                    });
                    p.spawn(button("Play", Action::Go(Screen::Setup), true));
                    if has_save() {
                        p.spawn(button("Continue saved game", Action::Load, true));
                    }
                    p.spawn(button("Settings", Action::OpenSettings, true));
                    p.spawn(button("Credits", Action::Go(Screen::Credits), true));
                    p.spawn(button("Quit", Action::Quit, true));
                    p.spawn((
                        Text::new(
                            "Based on OpenFront. Copyright (c) OpenFront and Contributors. AGPL-3.0.",
                        ),
                        font(11.0),
                        TextColor(TEXT_DIM),
                    ));
                }
                Screen::Setup => {
                    title(p, "New match", 30.0);
                    value_row(p, "Map", Field::Map);
                    p.spawn((Text::new(""), font(13.0), TextColor(TEXT_DIM), Field::MapInfo));
                    value_row(p, "Map size", Field::Size);
                    value_row(p, "Bots", Field::Bots);
                    value_row(p, "Nations", Field::Nations);
                    value_row(p, "Difficulty", Field::Difficulty);
                    value_row(p, "Mode", Field::Teams);
                    value_row(p, "Seed", Field::Seed);
                    p.spawn(Node {
                        column_gap: px(10),
                        ..default()
                    })
                    .with_children(|r| {
                        r.spawn(button("New seed", Action::NewSeed, false));
                        r.spawn(button("Suggested players", Action::UseSuggested, false));
                    });
                    p.spawn(Node {
                        column_gap: px(10),
                        margin: UiRect::top(px(8)),
                        ..default()
                    })
                    .with_children(|r| {
                        r.spawn(button("Back", Action::Go(Screen::MainMenu), false));
                        r.spawn(button("Start", Action::Start, false));
                    });
                }
                Screen::Settings => {
                    title(p, "Settings", 30.0);
                    value_row(p, "Sound volume", Field::Volume);
                    value_row(p, "Fullscreen", Field::Fullscreen);
                    value_row(p, "VSync", Field::Vsync);
                    value_row(p, "Interface size", Field::UiScale);
                    value_row(p, "Default attack size", Field::AttackSize);
                    value_row(p, "Tutorial", Field::Tutorial);
                    p.spawn(button("Done", Action::SettingsDone, true));
                }
                Screen::Credits => {
                    title(p, "Credits", 30.0);
                    for line in [
                        "laukofront is a Rust port of the OpenFront engine.",
                        "Copyright (c) OpenFront and Contributors - github.com/openfrontio/openfrontio",
                        "Licensed AGPL-3.0 with additional terms (see LICENSE, NOTICE.md).",
                        "Not affiliated with or endorsed by OpenFront.",
                        "",
                        "Game engine: Bevy (MIT or Apache-2.0).",
                        "Real-Earth coastlines: Natural Earth (public domain).",
                        "Icons, sounds, maps and colours: laukofront's own.",
                    ] {
                        p.spawn((Text::new(line), font(14.0), TextColor(TEXT)));
                    }
                    p.spawn(button("Back", Action::Go(Screen::MainMenu), true));
                }
                Screen::Paused => {
                    title(p, "Paused", 32.0);
                    p.spawn(button("Resume", Action::Resume, true));
                    p.spawn(button("Save game", Action::Save, true));
                    if has_save() {
                        p.spawn(button("Load saved game", Action::Load, true));
                    }
                    p.spawn(button("Restart this match", Action::Restart, true));
                    p.spawn(button("Settings", Action::OpenSettings, true));
                    p.spawn(button("Main menu", Action::Go(Screen::MainMenu), true));
                    p.spawn(button("Quit", Action::Quit, true));
                }
                Screen::Playing => {}
            });
        });
}

/// The match's panels (HUD, build bar, minimap…) are hidden behind the main menus and shown
/// while playing or paused.
#[allow(clippy::type_complexity)]
fn hide_game_ui(
    screen: Res<Screen>,
    back: Res<SettingsBack>,
    mut roots: Query<&mut Visibility, (With<Node>, Without<ChildOf>, Without<MenuRoot>)>,
) {
    let in_match = match *screen {
        Screen::Playing | Screen::Paused => true,
        Screen::Settings => back.0 == Screen::Paused,
        _ => false,
    };
    let v = if in_match {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut vis in roots.iter_mut() {
        if *vis != v {
            *vis = v;
        }
    }
}

fn update_values(
    draft: Res<SetupDraft>,
    saved: Res<SavedSettings>,
    mut texts: Query<(&Field, &mut Text)>,
) {
    for (f, mut t) in texts.iter_mut() {
        let s = field_text(*f, &draft.0, &saved);
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// The match pauses while the pause menu or settings from it are open.
fn pause_state(screen: Res<Screen>, back: Res<SettingsBack>, mut settings: ResMut<ClientSettings>) {
    if !screen.is_changed() {
        return;
    }
    let paused =
        *screen == Screen::Paused || (*screen == Screen::Settings && back.0 == Screen::Paused);
    if settings.paused != paused {
        settings.paused = paused;
    }
}

fn apply_settings_now(
    saved: Res<SavedSettings>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
    mut volume: ResMut<GlobalVolume>,
) {
    apply(&saved, &mut window, &mut scale, &mut volume);
}

fn apply_settings(
    saved: Res<SavedSettings>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
    mut volume: ResMut<GlobalVolume>,
) {
    if saved.is_changed() {
        apply(&saved, &mut window, &mut scale, &mut volume);
    }
}

fn apply(
    saved: &SavedSettings,
    window: &mut Window,
    scale: &mut UiScale,
    volume: &mut GlobalVolume,
) {
    let mode = if saved.fullscreen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    };
    if window.mode != mode {
        window.mode = mode;
    }
    // Automated runs (perf measurements) choose no vsync themselves; keep that.
    if window.present_mode != PresentMode::AutoNoVsync || saved.vsync {
        let pm = if saved.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
        if window.present_mode != pm {
            window.present_mode = pm;
        }
    }
    if (scale.0 - saved.ui_scale).abs() > 1e-3 {
        scale.0 = saved.ui_scale;
    }
    *volume = GlobalVolume::new(bevy::audio::Volume::Linear(saved.volume));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_wraps_and_clamps() {
        assert_eq!(steps(&[1, 2, 3], 3, 1), 1);
        assert_eq!(steps(&[1, 2, 3], 1, -1), 3);
        let d = SetupDraft::default().0;
        let s = SavedSettings::default();
        assert_eq!(field_text(Field::Teams, &d, &s), "free for all");
        assert_eq!(field_text(Field::Volume, &d, &s), "60%");
        let mut e = d;
        e.map = MapChoice::Earth(crate::engine_lib::earth::EarthMap::Europe);
        assert_eq!(field_text(Field::Size, &e, &s), "fixed");
        assert!(field_text(Field::MapInfo, &e, &s).contains("1200x1000"));
    }
}
