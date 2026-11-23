//! Milestone P10: alliances, traitors, embargoes, donations, targets, reactions, nation AI.

use laukofront::config::{Difficulty, GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::engine::attack_exec::WinCheckExecution;
use laukofront::engine::boats::TradeExecution;
use laukofront::engine::executions::PlayerExecution;
use laukofront::engine::player::Relation;
use laukofront::engine::units::UnitKind;
use laukofront::engine::weapons::SamExecution;
use laukofront::engine::{
    new_match, run_turn, Game, Intent, MatchSetup, PlayerId, PlayerInfo, PlayerType, HUMAN,
};
use laukofront::engine_lib::game_map::{terrain_byte, GameMap, TileRef};

const W: u32 = 300;
const H: u32 = 120;
const B: PlayerId = 2;

fn config() -> GameConfig {
    GameConfig::load(GAME_CONFIG_PATH).expect("game.toml")
}

/// One continent: x 2..=297 is land, split between the human (left) and B (right) at x = 150.
fn continent() -> GameMap {
    let land = |x: u32, y: u32| (2..H - 2).contains(&y) && (2..=297).contains(&x);
    let mut t = vec![terrain_byte(false, false, true, 0); (W * H) as usize];
    for y in 0..H {
        for x in 0..W {
            if land(x, y) {
                let shore = x == 2 || x == 297 || y == 2 || y == H - 3;
                t[(y * W + x) as usize] = terrain_byte(true, shore, false, 2);
            } else if (1..H - 1).contains(&y) && (1..=298).contains(&x) {
                t[(y * W + x) as usize] |= 1 << 6;
            }
        }
    }
    GameMap::new(W, H, t).unwrap()
}

fn t(x: u32, y: u32) -> TileRef {
    y * W + x
}

fn arena() -> Game {
    let mut g = Game::new(config(), continent(), "test");
    g.add_player(PlayerInfo::new("human", "You", PlayerType::Human));
    g.add_player(PlayerInfo::new("b", "B", PlayerType::Human));
    g.end_spawn_phase();
    for p in 1..=2 {
        g.add_execution(Box::new(PlayerExecution::new(p)));
    }
    g.add_execution(Box::new(WinCheckExecution::new()));
    g.add_execution(Box::new(TradeExecution::new(1)));
    g.add_execution(Box::new(SamExecution));
    for y in 2..H - 2 {
        for x in 2..150 {
            g.conquer(HUMAN, t(x, y));
        }
        for x in 150..=297 {
            g.conquer(B, t(x, y));
        }
    }
    while g.is_spawn_immunity_active() {
        run_turn(&mut g, &[]);
    }
    g
}

fn ally(g: &mut Game) {
    run_turn(
        g,
        &[Intent::AllianceRequest {
            player: HUMAN,
            to: B,
        }],
    );
    run_turn(
        g,
        &[Intent::AllianceReply {
            player: B,
            from: HUMAN,
            accept: true,
        }],
    );
    run_turn(g, &[]);
}

#[test]
fn request_and_accept_makes_allies_who_cannot_attack_each_other() {
    let mut g = arena();
    run_turn(
        &mut g,
        &[Intent::AllianceRequest {
            player: HUMAN,
            to: B,
        }],
    );
    run_turn(&mut g, &[]);
    assert!(g.has_request(HUMAN, B), "request pending");
    assert!(!g.can_send_alliance_request(HUMAN, B), "no duplicate");
    run_turn(
        &mut g,
        &[Intent::AllianceReply {
            player: B,
            from: HUMAN,
            accept: true,
        }],
    );
    run_turn(&mut g, &[]);
    assert!(g.is_allied(HUMAN, B));
    assert!(g.is_friendly(B, HUMAN));
    assert!(!g.can_attack_player(HUMAN, B));
    assert_eq!(g.player(B).unwrap().relation(HUMAN), Relation::Friendly);
    let tiles = g.player(B).unwrap().num_tiles();
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: Some(B),
            troops: Some(10_000.0),
        }],
    );
    for _ in 0..30 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(g.player(B).unwrap().num_tiles(), tiles, "ally attacked");
}

#[test]
fn asking_back_accepts_and_ongoing_attacks_retreat() {
    let mut g = arena();
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: Some(B),
            troops: Some(5_000.0),
        }],
    );
    run_turn(
        &mut g,
        &[Intent::AllianceRequest {
            player: B,
            to: HUMAN,
        }],
    );
    run_turn(
        &mut g,
        &[Intent::AllianceRequest {
            player: HUMAN,
            to: B,
        }],
    );
    run_turn(&mut g, &[]);
    assert!(g.is_allied(HUMAN, B), "a request back accepts");
    for _ in 0..5 {
        run_turn(&mut g, &[]);
    }
    assert!(
        g.player(HUMAN).unwrap().outgoing_attacks().is_empty(),
        "attack on the new ally retreated"
    );
}

#[test]
fn unanswered_requests_expire_and_have_a_cooldown() {
    let mut g = arena();
    run_turn(
        &mut g,
        &[Intent::AllianceRequest {
            player: HUMAN,
            to: B,
        }],
    );
    for _ in 0..205 {
        run_turn(&mut g, &[]);
    }
    assert!(!g.has_request(HUMAN, B), "expired after 20 s");
    assert!(!g.can_send_alliance_request(HUMAN, B), "cooldown");
    for _ in 0..100 {
        run_turn(&mut g, &[]);
    }
    assert!(g.can_send_alliance_request(HUMAN, B), "cooldown over");
}

#[test]
fn breaking_makes_a_traitor_for_30_seconds() {
    let mut g = arena();
    ally(&mut g);
    run_turn(
        &mut g,
        &[Intent::BreakAlliance {
            player: HUMAN,
            with: B,
        }],
    );
    run_turn(&mut g, &[]);
    assert!(!g.is_allied(HUMAN, B));
    assert!(g.is_traitor(HUMAN));
    assert!(!g.is_traitor(B));
    assert_eq!(g.player(HUMAN).unwrap().betrayals(), 1);
    assert!(
        g.player(B).unwrap().relation(HUMAN) < Relation::Friendly,
        "the betrayed one stops liking you"
    );
    for _ in 0..305 {
        run_turn(&mut g, &[]);
    }
    assert!(!g.is_traitor(HUMAN), "traitor mark lasts 30 s");
}

#[test]
fn traitors_defend_worse() {
    let attack = |traitor: bool| {
        let mut g = arena();
        if traitor {
            ally(&mut g);
            run_turn(
                &mut g,
                &[Intent::BreakAlliance {
                    player: B,
                    with: HUMAN,
                }],
            );
            run_turn(&mut g, &[]);
            assert!(g.is_traitor(B));
        }
        let before = g.player(B).unwrap().num_tiles();
        run_turn(
            &mut g,
            &[Intent::Attack {
                player: HUMAN,
                target: Some(B),
                troops: Some(20_000.0),
            }],
        );
        for _ in 0..40 {
            run_turn(&mut g, &[]);
        }
        before - g.player(B).unwrap().num_tiles()
    };
    let normal = attack(false);
    let traitor = attack(true);
    assert!(traitor > normal, "traitor lost {traitor}, normal {normal}");
}

#[test]
fn alliances_expire_unless_both_renew() {
    let mut g = arena();
    ally(&mut g);
    let expires = g.alliance_with(HUMAN, B).unwrap().expires;
    while g.ticks() + 100 < expires {
        run_turn(&mut g, &[]);
    }
    assert!(g.in_extension_window(g.alliance_with(HUMAN, B).unwrap()));
    run_turn(
        &mut g,
        &[Intent::RenewAlliance {
            player: HUMAN,
            with: B,
        }],
    );
    run_turn(&mut g, &[]);
    assert_eq!(
        g.alliance_with(HUMAN, B).unwrap().expires,
        expires,
        "one side only"
    );
    run_turn(
        &mut g,
        &[Intent::RenewAlliance {
            player: B,
            with: HUMAN,
        }],
    );
    run_turn(&mut g, &[]);
    let renewed = g.alliance_with(HUMAN, B).unwrap().expires;
    assert!(renewed > expires + 2_000, "renewed for 5 minutes");
    while g.ticks() <= renewed {
        run_turn(&mut g, &[]);
    }
    assert!(!g.is_allied(HUMAN, B), "expired");
    assert!(
        !g.is_traitor(HUMAN) && !g.is_traitor(B),
        "expiry is no betrayal"
    );
}

#[test]
fn donations_only_to_allies_with_cooldown() {
    let mut g = arena();
    let gold = g.player(B).unwrap().gold();
    run_turn(
        &mut g,
        &[Intent::DonateGold {
            player: HUMAN,
            to: B,
            gold: Some(100_000),
        }],
    );
    run_turn(&mut g, &[]);
    assert!(
        g.player(B).unwrap().gold() < gold + 100_000,
        "not allied: no gift"
    );
    ally(&mut g);
    g.player_mut(HUMAN).unwrap().add_gold(1_000_000);
    // Room under B's troop cap (donations never push past it, as upstream).
    g.player_mut(B).unwrap().set_troops(1_000.0);
    let theirs = g.player(B).unwrap().troops();
    run_turn(
        &mut g,
        &[Intent::DonateTroops {
            player: HUMAN,
            to: B,
            troops: Some(10_000.0),
        }],
    );
    run_turn(&mut g, &[]);
    // Your own troops grow back too fast to compare; the ally's count shows the gift.
    assert!(g.player(B).unwrap().troops() > theirs + 9_000);
    assert!(!g.can_donate(HUMAN, B), "10 s cooldown");
    for _ in 0..100 {
        run_turn(&mut g, &[]);
    }
    let gold = g.player(B).unwrap().gold();
    run_turn(
        &mut g,
        &[Intent::DonateGold {
            player: HUMAN,
            to: B,
            gold: None,
        }],
    );
    run_turn(&mut g, &[]);
    assert!(
        g.player(B).unwrap().gold() > gold + 300_000,
        "a third of the gold"
    );
}

#[test]
fn embargo_stops_trade_and_target_has_a_cooldown() {
    let mut g = arena();
    assert!(g.can_trade(HUMAN, B));
    run_turn(
        &mut g,
        &[Intent::Embargo {
            player: HUMAN,
            with: B,
            on: true,
        }],
    );
    run_turn(&mut g, &[]);
    assert!(!g.can_trade(HUMAN, B) && !g.can_trade(B, HUMAN));
    run_turn(
        &mut g,
        &[Intent::Embargo {
            player: HUMAN,
            with: B,
            on: false,
        }],
    );
    run_turn(&mut g, &[]);
    assert!(g.can_trade(HUMAN, B));
    run_turn(
        &mut g,
        &[Intent::Target {
            player: HUMAN,
            target: B,
        }],
    );
    run_turn(&mut g, &[]);
    assert_eq!(g.targets(HUMAN), vec![B]);
    assert!(!g.can_target(HUMAN, B), "15 s cooldown");
    for _ in 0..105 {
        run_turn(&mut g, &[]);
    }
    assert!(g.targets(HUMAN).is_empty(), "targets last 10 s");
    run_turn(
        &mut g,
        &[Intent::Reaction {
            player: HUMAN,
            to: None,
            what: 1,
        }],
    );
    run_turn(&mut g, &[]);
    assert_eq!(g.reactions().len(), 1);
    for _ in 0..55 {
        run_turn(&mut g, &[]);
    }
    assert!(g.reactions().is_empty(), "reactions fade after 5 s");
}

#[test]
fn nuking_an_ally_breaks_the_alliance() {
    let mut g = arena();
    ally(&mut g);
    g.player_mut(HUMAN).unwrap().add_gold(50_000_000);
    run_turn(
        &mut g,
        &[Intent::Build {
            player: HUMAN,
            kind: UnitKind::MissileSilo,
            tile: t(40, 60),
        }],
    );
    for _ in 0..UnitKind::MissileSilo.construction_ticks() + 2 {
        run_turn(&mut g, &[]);
    }
    run_turn(
        &mut g,
        &[Intent::Nuke {
            player: HUMAN,
            kind: UnitKind::AtomBomb,
            tile: t(250, 60),
        }],
    );
    run_turn(&mut g, &[]);
    assert!(!g.is_allied(HUMAN, B), "the launch ends the alliance");
    assert!(g.is_traitor(HUMAN));
    assert!(
        g.player(B).unwrap().relation(HUMAN) < Relation::Neutral,
        "the nuked ally turns against you"
    );
}

#[test]
fn allying_cancels_nukes_in_flight() {
    let mut g = arena();
    g.player_mut(HUMAN).unwrap().add_gold(50_000_000);
    run_turn(
        &mut g,
        &[Intent::Build {
            player: HUMAN,
            kind: UnitKind::MissileSilo,
            tile: t(10, 60),
        }],
    );
    for _ in 0..UnitKind::MissileSilo.construction_ticks() + 2 {
        run_turn(&mut g, &[]);
    }
    run_turn(
        &mut g,
        &[
            Intent::Nuke {
                player: HUMAN,
                kind: UnitKind::AtomBomb,
                tile: t(290, 60),
            },
            Intent::AllianceRequest {
                player: B,
                to: HUMAN,
            },
        ],
    );
    assert_eq!(g.unit_count(HUMAN, UnitKind::AtomBomb), 1);
    run_turn(
        &mut g,
        &[Intent::AllianceReply {
            player: HUMAN,
            from: B,
            accept: true,
        }],
    );
    run_turn(&mut g, &[]);
    assert!(g.is_allied(HUMAN, B));
    assert_eq!(g.unit_count(HUMAN, UnitKind::AtomBomb), 0, "nuke cancelled");
}

#[test]
fn difficulty_sets_nation_troops() {
    let easy = {
        let mut c = config();
        c.difficulty = Difficulty::Easy;
        c
    };
    let hard = {
        let mut c = config();
        c.difficulty = Difficulty::Impossible;
        c
    };
    use laukofront::engine::rules::{max_troops, start_manpower};
    assert_eq!(start_manpower(&easy, PlayerType::Nation), 12_500);
    assert_eq!(start_manpower(&hard, PlayerType::Nation), 31_250);
    assert_eq!(start_manpower(&hard, PlayerType::Human), 25_000);
    let h = max_troops(&easy, PlayerType::Human, 1000);
    assert!((max_troops(&easy, PlayerType::Nation, 1000) - h * 0.5).abs() < 1e-6);
    assert!((max_troops(&hard, PlayerType::Nation, 1000) - h * 1.25).abs() < 1e-6);
}

fn nation_match(seed: i64, d: Difficulty) -> Game {
    let mut g = new_match(
        config(),
        MatchSetup {
            size: MapSize::Small,
            seed,
            bots: 6,
            nations: 4,
            difficulty: d,
            map: Default::default(),
            teams: 0,
        },
    )
    .unwrap();
    while g.winner().is_none() && g.ticks() < 30_000 {
        run_turn(&mut g, &[]);
    }
    g
}

#[test]
fn nations_play_to_a_winner_and_build() {
    let g = nation_match(3, Difficulty::Medium);
    let w = g.winner().expect("a winner");
    assert_eq!(g.player(w).unwrap().kind(), PlayerType::Nation);
    let built = g
        .units()
        .iter()
        .filter(|u| {
            u.kind().is_structure() && g.player(u.owner()).unwrap().kind() == PlayerType::Nation
        })
        .count();
    assert!(built > 5, "nations built only {built} buildings");
    let names: Vec<&str> = g
        .players()
        .iter()
        .filter(|p| p.kind() == PlayerType::Nation)
        .map(|p| p.name())
        .collect();
    let mut uniq = names.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), names.len(), "unique nation names {names:?}");
}

#[test]
fn nation_matches_are_deterministic() {
    let a = nation_match(5, Difficulty::Hard);
    let b = nation_match(5, Difficulty::Hard);
    assert_eq!(a.ticks(), b.ticks());
    assert_eq!(a.state_hash(), b.state_hash());
}
