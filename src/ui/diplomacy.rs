//! Action menu and diplomacy UI. Right click any tile for upstream's radial-menu actions that fit
//! there: attack or send a boat (enemy or free land), build or upgrade (your land), a warship
//! (sea), and on players every diplomacy action. Also the diplomacy panel
//! (top right: alliance requests to answer, your alliances with time left and a renew button).
//! Notices for diplomacy events come from `client::diplo_notices`.

use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};
use bevy::window::PrimaryWindow;

use super::{fmt_num, font, ACCENT, DANGER, GOOD, PANEL, PANEL_BORDER, TEXT, TEXT_DIM};
use crate::client::{ClientSettings, GameState, MatchStats, Notices, PendingIntents};
use crate::engine::boats::can_send_boat;
use crate::engine::diplomacy::REACTIONS;
use crate::engine::player::Relation;
use crate::engine::structures::{build_cost, can_upgrade, placement};
use crate::engine::units::UnitKind;
use crate::engine::weapons::warship_spawn;
use crate::engine::{Game, Intent, PlayerId, HUMAN};
use crate::engine_lib::game_map::TileRef;
use crate::ui::buildbar::KEYS;

const REQUEST_ROWS: usize = 4;
const ALLY_ROWS: usize = 6;
/// Reactions offered in the menu (indices into `REACTIONS`).
const MENU_REACTIONS: [u8; 6] = [0, 1, 2, 3, 5, 9];

/// One button of the action menu.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    /// Land attack on the clicked player.
    Attack,
    /// Land attack into the clicked free land.
    Expand,
    /// Transport ship to the clicked land.
    Boat,
    /// Build on your land.
    Build(UnitKind),
    /// Upgrade your building next to the click.
    Upgrade,
    /// Warship patrolling the clicked water.
    Warship,
    Ally,
    Accept,
    Break,
    Renew,
    Embargo,
    StopEmbargo,
    SendTroops,
    SendGold,
    Target,
    React(u8),
    Close,
}

#[derive(Component)]
struct PlayerMenu;

#[derive(Component)]
struct MenuTitle;

#[derive(Component)]
struct MenuInfo;

/// The label text of a menu button (updated every frame: prices and troop counts change).
#[derive(Component)]
struct MenuLabel(MenuAction);

#[derive(Component)]
struct DiploPanel;

#[derive(Component, Clone, Copy)]
enum PanelRow {
    Request(usize),
    Ally(usize),
}

#[derive(Component, Clone, Copy)]
enum PanelText {
    Title,
    Request(usize),
    Ally(usize),
}

#[derive(Component, Clone, Copy)]
struct RenewButton(usize);

/// Who each panel row is about.
#[derive(Resource, Default)]
struct PanelIds {
    requests: Vec<PlayerId>,
    allies: Vec<PlayerId>,
}

pub struct DiplomacyUiPlugin;

impl Plugin for DiplomacyUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PanelIds>()
            .add_systems(Startup, (spawn_menu, spawn_panel))
            .add_systems(Update, (update_menu, update_panel));
    }
}

/// The other player whose land `tile` is (not you, not nobody).
fn other_owner(g: &Game, tile: TileRef) -> Option<PlayerId> {
    g.owner(tile).filter(|&o| o != HUMAN)
}

/// Troops an attack or boat from the menu sends.
fn send_troops(g: &Game, ratio: f64) -> f64 {
    g.player(HUMAN)
        .map_or(0.0, |p| (p.troops() as f64 * ratio).floor())
}

/// Your building within 2 tiles of `tile`, if any.
fn building_near(g: &Game, tile: TileRef) -> Option<crate::engine::units::UnitId> {
    let map = g.map();
    g.player_units(HUMAN, None)
        .filter(|u| u.kind().is_structure() && map.manhattan_dist(u.tile(), tile) <= 2)
        .min_by_key(|u| map.manhattan_dist(u.tile(), tile))
        .map(|u| u.id())
}

/// Which menu buttons apply to the clicked tile right now. Pure, for tests.
pub fn menu_actions(g: &Game, tile: TileRef, ratio: f64) -> Vec<MenuAction> {
    let mut out = Vec::new();
    let map = g.map();
    let alive = g.player(HUMAN).is_some_and(|p| p.is_alive());
    if !alive || g.in_spawn_phase() || g.winner().is_some() || !map.is_valid(tile) {
        out.push(MenuAction::Close);
        return out;
    }
    let troops_ok = send_troops(g, ratio) >= 1.0;
    if map.is_water(tile) {
        if warship_spawn(g, HUMAN, tile).is_ok() {
            out.push(MenuAction::Warship);
        }
    } else if g.owner(tile) == Some(HUMAN) {
        for (_, kind) in KEYS {
            if placement(g, HUMAN, kind, tile).is_ok() {
                out.push(MenuAction::Build(kind));
            }
        }
        if building_near(g, tile).is_some_and(|u| can_upgrade(g, HUMAN, u).is_ok()) {
            out.push(MenuAction::Upgrade);
        }
    } else if let Some(other) = other_owner(g, tile) {
        if !g.is_friendly(HUMAN, other) && g.can_attack_player(HUMAN, other) && troops_ok {
            if g.can_attack_tile(HUMAN, tile) {
                out.push(MenuAction::Attack);
            }
            if can_send_boat(g, HUMAN, tile).is_some() {
                out.push(MenuAction::Boat);
            }
        }
        if g.player(other).is_some_and(|p| p.is_alive()) {
            if g.has_request(other, HUMAN) {
                out.push(MenuAction::Accept);
            } else if g.can_send_alliance_request(HUMAN, other) {
                out.push(MenuAction::Ally);
            }
            if let Some(a) = g.alliance_with(HUMAN, other) {
                out.push(MenuAction::Break);
                if g.in_extension_window(a) && !a.agreed_to_extend(HUMAN) {
                    out.push(MenuAction::Renew);
                }
            }
            if g.player(HUMAN)
                .is_some_and(|p| p.has_embargo_against(other))
            {
                out.push(MenuAction::StopEmbargo);
            } else {
                out.push(MenuAction::Embargo);
            }
            if g.can_donate(HUMAN, other) {
                out.push(MenuAction::SendTroops);
                out.push(MenuAction::SendGold);
            }
            if g.can_target(HUMAN, other) {
                out.push(MenuAction::Target);
            }
            out.extend(MENU_REACTIONS.iter().map(|&r| MenuAction::React(r)));
        }
    } else if map.is_passable_land(tile) && troops_ok {
        if g.can_attack_tile(HUMAN, tile) {
            out.push(MenuAction::Expand);
        }
        if can_send_boat(g, HUMAN, tile).is_some() {
            out.push(MenuAction::Boat);
        }
    }
    out.push(MenuAction::Close);
    out
}

fn kind_name(k: UnitKind) -> &'static str {
    match k {
        UnitKind::City => "city",
        UnitKind::Port => "port",
        UnitKind::Factory => "factory",
        UnitKind::DefensePost => "defense post",
        UnitKind::MissileSilo => "missile silo",
        UnitKind::SamLauncher => "SAM launcher",
        _ => "building",
    }
}

/// The text of a menu button. Pure, for tests.
pub fn action_label(a: MenuAction, g: &Game, ratio: f64) -> String {
    let troops = fmt_num(send_troops(g, ratio));
    let pct = (ratio * 100.0).round();
    match a {
        MenuAction::Attack => format!("Attack with {pct}% ({troops} troops)"),
        MenuAction::Expand => format!("Expand here with {pct}% ({troops})"),
        MenuAction::Boat => format!("Send boat with {pct}% ({troops} troops)"),
        MenuAction::Build(k) => format!(
            "Build {} ({})",
            kind_name(k),
            fmt_num(build_cost(g, HUMAN, k) as f64)
        ),
        MenuAction::Upgrade => "Upgrade this building".into(),
        MenuAction::Warship => format!(
            "Build warship here ({})",
            fmt_num(build_cost(g, HUMAN, UnitKind::Warship) as f64)
        ),
        MenuAction::Ally => "Ask for alliance".into(),
        MenuAction::Accept => "Accept alliance".into(),
        MenuAction::Break => "Break alliance (traitor!)".into(),
        MenuAction::Renew => "Renew alliance".into(),
        MenuAction::Embargo => "Stop trading (embargo)".into(),
        MenuAction::StopEmbargo => "Trade again".into(),
        MenuAction::SendTroops => "Send 1/3 of troops".into(),
        MenuAction::SendGold => "Send 1/3 of gold".into(),
        MenuAction::Target => "Ask allies to attack them".into(),
        MenuAction::React(r) => REACTIONS[r as usize].into(),
        MenuAction::Close => "Close".into(),
    }
}

/// The intent (and notice) a menu button sends for the clicked tile.
pub fn action_intent(
    a: MenuAction,
    g: &Game,
    tile: TileRef,
    ratio: f64,
) -> Option<(Intent, String)> {
    let p = HUMAN;
    let troops = send_troops(g, ratio);
    let other = other_owner(g, tile);
    let need_other = || other.ok_or(());
    let t = |s: &str| s.to_string();
    Some(match a {
        MenuAction::Attack => (
            Intent::Attack {
                player: p,
                target: other,
                troops: Some(troops),
            },
            format!("Attacking with {} troops", fmt_num(troops)),
        ),
        MenuAction::Expand => (
            Intent::Attack {
                player: p,
                target: None,
                troops: Some(troops),
            },
            String::new(),
        ),
        MenuAction::Boat => {
            let (src, dst) = can_send_boat(g, HUMAN, tile)?;
            let secs = (g.map().manhattan_dist(src, dst) as f64 * 0.8
                / g.config().game.tick_hz as f64)
                .round();
            (
                Intent::Boat {
                    player: p,
                    tile,
                    troops: Some(troops),
                },
                format!(
                    "Boat sailing with {} troops, lands in about {secs:.0} s",
                    fmt_num(troops)
                ),
            )
        }
        MenuAction::Build(kind) => (
            Intent::Build {
                player: p,
                kind,
                tile: placement(g, HUMAN, kind, tile).ok()?,
            },
            format!("Building a {}", kind_name(kind)),
        ),
        MenuAction::Upgrade => (
            Intent::Upgrade {
                player: p,
                unit: building_near(g, tile)?,
            },
            t("Upgrading"),
        ),
        MenuAction::Warship => (Intent::Warship { player: p, tile }, t("Warship ordered")),
        MenuAction::Ally => (
            Intent::AllianceRequest {
                player: p,
                to: need_other().ok()?,
            },
            t("Alliance request sent"),
        ),
        MenuAction::Accept => (
            Intent::AllianceReply {
                player: p,
                from: need_other().ok()?,
                accept: true,
            },
            t("Alliance accepted"),
        ),
        MenuAction::Break => (
            Intent::BreakAlliance {
                player: p,
                with: need_other().ok()?,
            },
            t("Alliance broken: you are a traitor for 30 s"),
        ),
        MenuAction::Renew => (
            Intent::RenewAlliance {
                player: p,
                with: need_other().ok()?,
            },
            t("Asked to renew the alliance"),
        ),
        MenuAction::Embargo => (
            Intent::Embargo {
                player: p,
                with: need_other().ok()?,
                on: true,
            },
            t("Embargo: no more trade with them"),
        ),
        MenuAction::StopEmbargo => (
            Intent::Embargo {
                player: p,
                with: need_other().ok()?,
                on: false,
            },
            t("Trading again"),
        ),
        MenuAction::SendTroops => (
            Intent::DonateTroops {
                player: p,
                to: need_other().ok()?,
                troops: None,
            },
            t("Troops sent"),
        ),
        MenuAction::SendGold => (
            Intent::DonateGold {
                player: p,
                to: need_other().ok()?,
                gold: None,
            },
            t("Gold sent"),
        ),
        MenuAction::Target => (
            Intent::Target {
                player: p,
                target: need_other().ok()?,
            },
            t("Your allies will attack them"),
        ),
        MenuAction::React(r) => (
            Intent::Reaction {
                player: p,
                to: other,
                what: r,
            },
            String::new(),
        ),
        MenuAction::Close => return None,
    })
}

fn all_actions() -> Vec<MenuAction> {
    let mut v = vec![MenuAction::Attack, MenuAction::Expand, MenuAction::Boat];
    v.extend(KEYS.iter().map(|&(_, k)| MenuAction::Build(k)));
    v.extend([
        MenuAction::Upgrade,
        MenuAction::Warship,
        MenuAction::Accept,
        MenuAction::Ally,
        MenuAction::Break,
        MenuAction::Renew,
        MenuAction::Embargo,
        MenuAction::StopEmbargo,
        MenuAction::SendTroops,
        MenuAction::SendGold,
        MenuAction::Target,
    ]);
    v.extend(MENU_REACTIONS.iter().map(|&r| MenuAction::React(r)));
    v.push(MenuAction::Close);
    v
}

fn button_color(a: MenuAction) -> Color {
    match a {
        MenuAction::Attack | MenuAction::Expand | MenuAction::Boat => DANGER.with_alpha(0.32),
        MenuAction::Build(_) | MenuAction::Upgrade | MenuAction::Warship => ACCENT.with_alpha(0.22),
        MenuAction::Ally | MenuAction::Accept | MenuAction::Renew => GOOD.with_alpha(0.28),
        MenuAction::Break => DANGER.with_alpha(0.3),
        MenuAction::React(_) => Color::srgba(1.0, 1.0, 1.0, 0.07),
        _ => Color::srgba(1.0, 1.0, 1.0, 0.1),
    }
}

fn menu_button(a: MenuAction) -> impl Bundle {
    (
        Button,
        Node {
            padding: UiRect::axes(px(6), px(3)),
            border_radius: BorderRadius::all(px(4)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(button_color(a)),
        a,
        observe(
            move |_: On<Activate>,
                  mut s: ResMut<ClientSettings>,
                  game: Res<GameState>,
                  mut p: ResMut<PendingIntents>,
                  mut n: ResMut<Notices>| {
                let Some(tile) = s.menu_tile else { return };
                if let Some((intent, text)) = action_intent(a, &game.0, tile, s.attack_ratio) {
                    p.0.push(intent);
                    if !text.is_empty() {
                        n.push(text);
                    }
                }
                s.menu_tile = None;
            },
        ),
        children![(Text::new(""), font(12.0), TextColor(TEXT), MenuLabel(a))],
    )
}

fn spawn_menu(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: px(250),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(8)),
                row_gap: px(3),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(PANEL_BORDER),
            GlobalZIndex(20),
            PlayerMenu,
        ))
        .with_children(|m| {
            m.spawn((Text::new(""), font(15.0), TextColor(ACCENT), MenuTitle));
            m.spawn((Text::new(""), font(12.0), TextColor(TEXT_DIM), MenuInfo));
            for a in all_actions() {
                if !matches!(a, MenuAction::React(_) | MenuAction::Close) {
                    m.spawn(menu_button(a));
                }
            }
            m.spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(3),
                row_gap: px(3),
                margin: UiRect::top(px(3)),
                ..default()
            })
            .with_children(|row| {
                for &r in &MENU_REACTIONS {
                    row.spawn(menu_button(MenuAction::React(r)));
                }
            });
            m.spawn(menu_button(MenuAction::Close));
        });
}

fn relation_text(r: Relation) -> &'static str {
    match r {
        Relation::Hostile => "hostile",
        Relation::Distrustful => "distrustful",
        Relation::Neutral => "neutral",
        Relation::Friendly => "friendly",
    }
}

/// The info lines of the player menu. Pure, for tests.
pub fn menu_info(g: &Game, other: PlayerId) -> String {
    let Some(p) = g.player(other) else {
        return String::new();
    };
    let mut lines = vec![format!(
        "{} troops  {} gold  {} tiles",
        fmt_num(p.troops() as f64),
        fmt_num(p.gold() as f64),
        fmt_num(p.num_tiles() as f64)
    )];
    let mut status = Vec::new();
    if let Some(a) = g.alliance_with(HUMAN, other) {
        let secs = a.expires.saturating_sub(g.ticks()) / g.config().game.tick_hz as u64;
        status.push(format!("your ally ({}:{:02} left)", secs / 60, secs % 60));
    }
    if g.same_team(HUMAN, other) {
        status.push("your teammate".into());
    }
    if g.has_request(HUMAN, other) {
        status.push("request sent".into());
    }
    if g.is_traitor(other) {
        status.push("TRAITOR".into());
    }
    if g.player(HUMAN)
        .is_some_and(|me| me.has_embargo_against(other))
    {
        status.push("embargoed by you".into());
    }
    if p.has_embargo_against(HUMAN) {
        status.push("embargoes you".into());
    }
    status.push(format!(
        "feels {} toward you",
        relation_text(p.relation(HUMAN))
    ));
    lines.push(status.join(", "));
    lines.join("\n")
}

/// Title and info of the menu for a tile. Pure, for tests.
pub fn menu_header(g: &Game, tile: TileRef) -> (String, String) {
    let map = g.map();
    if map.is_water(tile) {
        return ("Sea".into(), "Warships patrol here.".into());
    }
    match g.owner(tile) {
        Some(HUMAN) => (
            "Your land".into(),
            format!(
                "{} gold to build with",
                fmt_num(g.player(HUMAN).map_or(0, |p| p.gold()) as f64)
            ),
        ),
        Some(o) => (
            g.player(o).map_or("?", |p| p.name()).to_string(),
            menu_info(g, o),
        ),
        None if map.is_impassable(tile) => ("Mountains".into(), "Nobody can take this.".into()),
        None => ("Free land".into(), "Nobody owns this yet.".into()),
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_menu(
    game: Res<GameState>,
    mut settings: ResMut<ClientSettings>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut menu: Query<&mut Node, With<PlayerMenu>>,
    mut title: Query<&mut Text, (With<MenuTitle>, Without<MenuInfo>, Without<MenuLabel>)>,
    mut info: Query<&mut Text, (With<MenuInfo>, Without<MenuTitle>, Without<MenuLabel>)>,
    mut labels: Query<(&MenuLabel, &mut Text), (Without<MenuTitle>, Without<MenuInfo>)>,
    mut buttons: Query<(&MenuAction, &mut Node), Without<PlayerMenu>>,
    stats: Res<MatchStats>,
) {
    let g = &game.0;
    if stats.ended && settings.menu_tile.is_some() {
        settings.menu_tile = None;
    }
    let Ok(mut node) = menu.single_mut() else {
        return;
    };
    let Some(tile) = settings.menu_tile else {
        if node.display != Display::None {
            node.display = Display::None;
        }
        return;
    };
    node.display = Display::Flex;
    let (w, h) = (window.width(), window.height());
    let pos = settings.menu_pos;
    node.left = px(pos.x.min(w - 260.0).max(4.0));
    node.top = px(pos.y.min(h - 420.0).max(4.0));
    let (t, i) = menu_header(g, tile);
    for mut x in title.iter_mut() {
        if x.0 != t {
            x.0 = t.clone();
        }
    }
    for mut x in info.iter_mut() {
        if x.0 != i {
            x.0 = i.clone();
        }
    }
    let show = menu_actions(g, tile, settings.attack_ratio);
    for (a, mut n) in buttons.iter_mut() {
        let d = if show.contains(a) {
            Display::Flex
        } else {
            Display::None
        };
        if n.display != d {
            n.display = d;
        }
    }
    for (l, mut x) in labels.iter_mut() {
        if show.contains(&l.0) {
            let s = action_label(l.0, g, settings.attack_ratio);
            if x.0 != s {
                x.0 = s;
            }
        }
    }
}

fn small_button(text: &str, color: Color) -> impl Bundle {
    (
        Button,
        Node {
            padding: UiRect::axes(px(6), px(1)),
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BackgroundColor(color),
        children![(Text::new(text.to_string()), font(11.0), TextColor(TEXT))],
    )
}

fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(56),
                width: px(270),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(px(10), px(8)),
                row_gap: px(4),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(PANEL_BORDER),
            DiploPanel,
        ))
        .with_children(|c| {
            c.spawn((
                Text::new("Diplomacy"),
                font(13.0),
                TextColor(TEXT_DIM),
                PanelText::Title,
            ));
            for i in 0..REQUEST_ROWS {
                c.spawn((
                    Node {
                        width: percent(100),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        column_gap: px(4),
                        ..default()
                    },
                    PanelRow::Request(i),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(""),
                        font(12.0),
                        TextColor(ACCENT),
                        PanelText::Request(i),
                    ));
                    row.spawn(Node {
                        column_gap: px(4),
                        ..default()
                    })
                    .with_children(|b| {
                        for accept in [true, false] {
                            b.spawn((
                                small_button(
                                    if accept { "accept" } else { "reject" },
                                    if accept {
                                        GOOD.with_alpha(0.3)
                                    } else {
                                        DANGER.with_alpha(0.25)
                                    },
                                ),
                                observe(
                                    move |_: On<Activate>,
                                          ids: Res<PanelIds>,
                                          mut p: ResMut<PendingIntents>| {
                                        if let Some(&from) = ids.requests.get(i) {
                                            p.0.push(Intent::AllianceReply {
                                                player: HUMAN,
                                                from,
                                                accept,
                                            });
                                        }
                                    },
                                ),
                            ));
                        }
                    });
                });
            }
            for i in 0..ALLY_ROWS {
                c.spawn((
                    Node {
                        width: percent(100),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    PanelRow::Ally(i),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(""),
                        font(12.0),
                        TextColor(TEXT),
                        PanelText::Ally(i),
                    ));
                    row.spawn((
                        small_button("renew", GOOD.with_alpha(0.3)),
                        RenewButton(i),
                        observe(
                            move |_: On<Activate>,
                                  ids: Res<PanelIds>,
                                  mut p: ResMut<PendingIntents>,
                                  mut n: ResMut<Notices>| {
                                if let Some(&with) = ids.allies.get(i) {
                                    p.0.push(Intent::RenewAlliance {
                                        player: HUMAN,
                                        with,
                                    });
                                    n.push("Asked to renew the alliance");
                                }
                            },
                        ),
                    ));
                });
            }
        });
}

/// Rows of the diplomacy panel: (incoming requests, allies with "name m:ss" and renew shown).
/// Pure, for tests.
/// Requests (who, text) and alliances (who, text, renew button shown).
pub type PanelRows = (Vec<(PlayerId, String)>, Vec<(PlayerId, String, bool)>);

pub fn panel_rows(g: &Game) -> PanelRows {
    let reqs = g
        .incoming_requests(HUMAN)
        .map(|r| {
            let name = g.player(r.requestor).map_or("?", |p| p.name());
            (r.requestor, format!("{name} asks to ally"))
        })
        .collect();
    let hz = g.config().game.tick_hz as u64;
    let allies = g
        .alliances()
        .iter()
        .filter(|a| a.involves(HUMAN))
        .map(|a| {
            let o = a.other(HUMAN);
            let name = g.player(o).map_or("?", |p| p.name());
            let secs = a.expires.saturating_sub(g.ticks()) / hz;
            let mut s = format!("Ally {name}  {}:{:02}", secs / 60, secs % 60);
            if a.agreed_to_extend(HUMAN) {
                s.push_str("  (asked)");
            } else if a.agreed_to_extend(o) {
                s.push_str("  wants renewal");
            }
            let renew = g.in_extension_window(a) && !a.agreed_to_extend(HUMAN);
            (o, s, renew)
        })
        .collect();
    (reqs, allies)
}

#[allow(clippy::type_complexity)]
fn update_panel(
    game: Res<GameState>,
    mut ids: ResMut<PanelIds>,
    mut panel: Query<&mut Node, (With<DiploPanel>, Without<PanelRow>, Without<RenewButton>)>,
    mut rows: Query<(&PanelRow, &mut Node), (Without<DiploPanel>, Without<RenewButton>)>,
    mut renew: Query<(&RenewButton, &mut Node), (Without<DiploPanel>, Without<PanelRow>)>,
    mut texts: Query<(&PanelText, &mut Text)>,
) {
    let (reqs, allies) = panel_rows(&game.0);
    ids.requests = reqs.iter().map(|r| r.0).collect();
    ids.allies = allies.iter().map(|a| a.0).collect();
    let any = !reqs.is_empty() || !allies.is_empty();
    for mut n in panel.iter_mut() {
        let d = if any { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    for (r, mut n) in rows.iter_mut() {
        let show = match *r {
            PanelRow::Request(i) => i < reqs.len(),
            PanelRow::Ally(i) => i < allies.len(),
        };
        let d = if show { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    for (b, mut n) in renew.iter_mut() {
        let show = allies.get(b.0).is_some_and(|a| a.2);
        let d = if show { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    for (t, mut text) in texts.iter_mut() {
        let s = match *t {
            PanelText::Title => continue,
            PanelText::Request(i) => reqs.get(i).map(|r| r.1.clone()).unwrap_or_default(),
            PanelText::Ally(i) => allies.get(i).map(|a| a.1.clone()).unwrap_or_default(),
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
    use crate::engine::{new_match, run_turn, MatchSetup};

    fn started() -> (Game, PlayerId) {
        let cfg = GameConfig::load(GAME_CONFIG_PATH).unwrap();
        let mut setup = MatchSetup::bots_only(MapSize::Small, 4, 0);
        setup.nations = 2;
        let mut g = new_match(cfg, setup).unwrap();
        run_turn(&mut g, &[]);
        let map = g.map();
        let tile = (0..map.num_tiles())
            .find(|&t| crate::engine::util::spawn_tiles(map, t, 4, true).is_some())
            .unwrap();
        run_turn(
            &mut g,
            &[Intent::Spawn {
                player: HUMAN,
                tile,
            }],
        );
        while g.in_spawn_phase() {
            run_turn(&mut g, &[]);
        }
        run_turn(&mut g, &[]);
        let other = g.players().iter().find(|p| p.id() != HUMAN).unwrap().id();
        (g, other)
    }

    #[test]
    fn menu_offers_alliance_then_break_and_donations() {
        let (mut g, other) = started();
        let at = g.player(other).unwrap().tiles()[0];
        let acts = menu_actions(&g, at, 0.2);
        assert!(acts.contains(&MenuAction::Ally));
        assert!(!acts.contains(&MenuAction::Break));
        assert!(!acts.contains(&MenuAction::SendTroops));
        // Make them ask us, then accept from the menu's intent.
        run_turn(
            &mut g,
            &[Intent::AllianceRequest {
                player: other,
                to: HUMAN,
            }],
        );
        run_turn(&mut g, &[]);
        let at = g.player(other).unwrap().tiles()[0];
        assert!(menu_actions(&g, at, 0.2).contains(&MenuAction::Accept));
        assert_eq!(panel_rows(&g).0.len(), 1);
        let (intent, _) = action_intent(MenuAction::Accept, &g, at, 0.2).unwrap();
        run_turn(&mut g, &[intent]);
        run_turn(&mut g, &[]);
        let at = g.player(other).unwrap().tiles()[0];
        let acts = menu_actions(&g, at, 0.2);
        assert!(acts.contains(&MenuAction::Break));
        assert!(acts.contains(&MenuAction::SendGold));
        assert!(
            !acts.contains(&MenuAction::Attack),
            "allies cannot be attacked"
        );
        assert!(!acts.contains(&MenuAction::Boat), "or boarded");
        assert!(
            !acts.contains(&MenuAction::Target),
            "allies cannot be targeted"
        );
        assert!(menu_info(&g, other).contains("your ally"));
        assert_eq!(panel_rows(&g).1.len(), 1);
    }

    #[test]
    fn menu_offers_building_on_your_land_and_expanding_next_to_it() {
        let (mut g, _) = started();
        g.player_mut(HUMAN).unwrap().add_gold(10_000_000);
        let mine = g.player(HUMAN).unwrap().tiles()[0];
        let acts = menu_actions(&g, mine, 0.2);
        assert!(
            acts.contains(&MenuAction::Build(UnitKind::City)),
            "{acts:?}"
        );
        let (intent, text) =
            action_intent(MenuAction::Build(UnitKind::City), &g, mine, 0.2).unwrap();
        assert!(matches!(
            intent,
            Intent::Build {
                kind: UnitKind::City,
                ..
            }
        ));
        assert!(text.contains("city"));
        // Free land next to your border: expand.
        let map = g.map().clone();
        let mut buf = [0; 4];
        let free = g
            .player(HUMAN)
            .unwrap()
            .border_tiles()
            .iter()
            .flat_map(|&b| {
                let n = map.neighbors4(b, &mut buf);
                buf[..n].to_vec()
            })
            .find(|&t| map.is_passable_land(t) && !map.has_owner(t))
            .unwrap();
        assert!(menu_actions(&g, free, 0.2).contains(&MenuAction::Expand));
        assert_eq!(menu_header(&g, free).0, "Free land");
        assert!(action_label(MenuAction::Expand, &g, 0.2).contains("20%"));
    }
}
