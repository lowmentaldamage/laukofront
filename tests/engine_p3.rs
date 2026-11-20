//! Milestone P3: attacks, retreat, defeated-player takeover, encirclement, win check.
//! Most tests use a small hand-built map (plains inside a one-tile ocean ring) so territories can
//! be laid out exactly.

use std::collections::HashSet;

use laukofront::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::engine::attack_exec::WinCheckExecution;
use laukofront::engine::executions::PlayerExecution;
use laukofront::engine::rules::{self, AttackLogicInput, DefenderInfo};
use laukofront::engine::util::spawn_tiles;
use laukofront::engine::{
    new_match, run_turn, Game, Intent, MatchSetup, PlayerId, PlayerInfo, PlayerType, HUMAN,
};
use laukofront::engine_lib::detmath::{log, sigmoid, within};
use laukofront::engine_lib::game_map::{terrain_byte, GameMap, TerrainType, TileRef};
use laukofront::engine_lib::random::PseudoRandom;

const W: u32 = 100;
const H: u32 = 60;

fn config() -> GameConfig {
    GameConfig::load(GAME_CONFIG_PATH).expect("game.toml")
}

/// Plains with a one-tile ocean ring.
fn plains_map() -> GameMap {
    let mut t = vec![terrain_byte(true, false, false, 2); (W * H) as usize];
    for y in 0..H {
        for x in 0..W {
            if x == 0 || y == 0 || x == W - 1 || y == H - 1 {
                t[(y * W + x) as usize] = terrain_byte(false, false, true, 0);
            }
        }
    }
    GameMap::new(W, H, t).unwrap()
}

/// Human (1) and two bots (2, 3), spawn phase over, economies and win check running.
fn arena() -> Game {
    let mut g = Game::new(config(), plains_map(), "test");
    g.add_player(PlayerInfo::new("human", "You", PlayerType::Human));
    g.add_player(PlayerInfo::new("bot_a", "A", PlayerType::Bot));
    g.add_player(PlayerInfo::new("bot_b", "B", PlayerType::Bot));
    g.end_spawn_phase();
    for p in 1..=3 {
        g.add_execution(Box::new(PlayerExecution::new(p)));
    }
    g.add_execution(Box::new(WinCheckExecution::new()));
    g
}

fn give_rect(g: &mut Game, p: PlayerId, x0: u32, y0: u32, x1: u32, y1: u32) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            let t = g.map().tile(x as i64, y as i64).unwrap();
            g.conquer(p, t);
        }
    }
}

/// Runs ticks until spawn immunity is over.
fn past_immunity(g: &mut Game) {
    while g.is_spawn_immunity_active() {
        run_turn(g, &[]);
    }
}

fn check_invariants(g: &Game) {
    let map = g.map();
    let mut counts = vec![0u32; g.players().len() + 1];
    for t in 0..map.num_tiles() {
        let o = map.owner_id(t) as usize;
        if o != 0 {
            assert!(map.is_passable_land(t));
            counts[o] += 1;
        }
    }
    for p in g.players() {
        assert_eq!(p.num_tiles(), counts[p.id() as usize], "{}", p.name());
        let border: HashSet<TileRef> = p.border_tiles().iter().copied().collect();
        let expect: HashSet<TileRef> = p
            .tiles()
            .iter()
            .copied()
            .filter(|&t| map.is_border(t))
            .collect();
        assert_eq!(border, expect, "{} border", p.name());
        assert!(p.troops() >= 0 && p.gold() >= 0);
        for &a in p.outgoing_attacks() {
            let at = g.attack(a).unwrap();
            assert!(at.is_active() && at.attacker() == p.id() && at.troops() >= 0.0);
        }
    }
}

// ----- attack_logic formula -----

#[test]
fn attack_logic_matches_upstream_formula_against_a_player() {
    let c = config();
    let input = AttackLogicInput {
        terrain: TerrainType::Highland,
        attack_troops: 10_000.0,
        attacker_kind: PlayerType::Human,
        attacker_tiles: 5_000,
        defender: Some(DefenderInfo {
            kind: PlayerType::Human,
            num_tiles: 2_000,
            troops: 30_000.0,
            is_traitor: false,
            is_disconnected_teammate: false,
        }),
        defender_has_defense_post: false,
        fallout_ratio: None,
        border_size: 40.0,
    };
    let r = rules::attack_logic(&c, &input).unwrap();
    let bonus = |tiles: f64, depth: f64| 1.0 - depth * sigmoid(log(tiles), 2.5, log(300_000.0));
    let (mag, cost) = (100.0, 20.0);
    let ratio: f64 = 30_000.0 / 10_000.0;
    let la = bonus(5_000.0, 0.7);
    let ld = bonus(2_000.0, 0.3);
    let density = 30_000.0 / 2_000.0;
    let loss = mag * within(ratio, 0.6, 2.0) * (0.463 * la * ld + 0.0039 * density);
    let speed = within(ratio, 0.82, 7.5) * within(ratio / 20.0, 1.0, 50.0) / 8.55;
    let frac = speed * cost * bonus(5_000.0, 0.73) * ld / 40.0;
    assert_eq!(r.attacker_troop_loss, loss);
    assert_eq!(r.defender_troop_loss, density);
    assert_eq!(r.tick_fraction, frac);
}

#[test]
fn attack_logic_unowned_land_and_bots() {
    let c = config();
    let mut input = AttackLogicInput {
        terrain: TerrainType::Plains,
        attack_troops: 1_000.0,
        attacker_kind: PlayerType::Human,
        attacker_tiles: 100,
        defender: None,
        defender_has_defense_post: false,
        fallout_ratio: None,
        border_size: 10.0,
    };
    let r = rules::attack_logic(&c, &input).unwrap();
    assert_eq!(r.attacker_troop_loss, 80.0 / 5.0);
    assert_eq!(r.defender_troop_loss, 0.0);
    // clamp(2000 * 16.5 / 1000, 5, 100) / (2 * 10) = 33 / 20
    assert_eq!(r.tick_fraction, 33.0 / 20.0);
    input.attacker_kind = PlayerType::Bot;
    assert_eq!(
        rules::attack_logic(&c, &input).unwrap().attacker_troop_loss,
        8.0
    );
    input.terrain = TerrainType::Ocean;
    assert!(rules::attack_logic(&c, &input).is_none());
}

// ----- attacks in a game -----

#[test]
fn attack_on_unowned_land_expands_and_costs_troops() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 10, 10, 14, 14);
    let start_tiles = g.player(HUMAN).unwrap().num_tiles();
    let troops = g.player(HUMAN).unwrap().troops();
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: None,
            troops: Some(5_000.0),
        }],
    );
    // Initialised at the end of that tick: troops left the pool, an attack exists.
    let p = g.player(HUMAN).unwrap();
    assert_eq!(p.outgoing_attacks().len(), 1);
    assert!(p.troops() <= troops - 5_000 + 200, "troops {}", p.troops());
    for _ in 0..30 {
        run_turn(&mut g, &[]);
        check_invariants(&g);
    }
    let grown = g.player(HUMAN).unwrap().num_tiles();
    assert!(grown > start_tiles + 50, "tiles {start_tiles} -> {grown}");
}

#[test]
fn default_attack_sends_twenty_percent() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 10, 10, 14, 14);
    let before = g.player(HUMAN).unwrap().troops();
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: None,
            troops: None,
        }],
    );
    let id = g.player(HUMAN).unwrap().outgoing_attacks()[0];
    assert_eq!(
        g.attack(id).unwrap().troops(),
        (before as f64 * 0.2).floor()
    );
}

#[test]
fn attack_on_a_bot_takes_its_land_and_kills_its_troops() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 5, 5, 40, 50);
    give_rect(&mut g, 2, 41, 5, 90, 50);
    past_immunity(&mut g);
    let bot_tiles = g.player(2).unwrap().num_tiles();
    let bot_troops = g.player(2).unwrap().troops();
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: Some(2),
            troops: Some(20_000.0),
        }],
    );
    for _ in 0..40 {
        run_turn(&mut g, &[]);
        check_invariants(&g);
    }
    let bot = g.player(2).unwrap();
    assert!(bot.num_tiles() < bot_tiles, "bot lost no land");
    assert!(bot.troops() < bot_troops + 2_000, "bot lost no troops");
}

#[test]
fn attack_without_a_shared_border_refunds_everything() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 5, 5, 10, 10);
    give_rect(&mut g, 2, 60, 30, 70, 40);
    past_immunity(&mut g);
    let before = g.player(HUMAN).unwrap().troops();
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: Some(2),
            troops: Some(1_000.0),
        }],
    );
    let p = g.player(HUMAN).unwrap();
    assert!(p.outgoing_attacks().is_empty());
    // Refunded in full; economy may add troops on the same tick, never remove them.
    assert!(p.troops() >= before);
}

#[test]
fn humans_cannot_attack_during_spawn_immunity_but_bots_can() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 5, 5, 40, 50);
    give_rect(&mut g, 2, 41, 5, 90, 50);
    assert!(g.is_spawn_immunity_active());
    assert!(g.can_attack_player(HUMAN, 2), "bots are never immune");
    assert!(
        g.can_attack_player(2, HUMAN),
        "bot attackers ignore immunity"
    );
    assert!(!g.can_attack_player(HUMAN, HUMAN));
    // A human attacking a human is blocked while immunity lasts.
    g.add_player(PlayerInfo::new("h2", "H2", PlayerType::Human));
    assert!(!g.can_attack_player(HUMAN, 4));
    past_immunity(&mut g);
    assert!(g.can_attack_player(HUMAN, 4));
}

#[test]
fn retreat_returns_troops_minus_malus_after_delay() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 5, 5, 40, 50);
    give_rect(&mut g, 2, 41, 5, 90, 50);
    // A huge defender so the attack barely moves.
    g.player_mut(2).unwrap().set_troops(5_000_000.0);
    past_immunity(&mut g);
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: Some(2),
            troops: Some(10_000.0),
        }],
    );
    let id = g.player(HUMAN).unwrap().outgoing_attacks()[0];
    run_turn(
        &mut g,
        &[Intent::CancelAttack {
            player: HUMAN,
            attack: id,
        }],
    );
    run_turn(&mut g, &[]); // retreat ordered: the attack stops moving
    assert!(g.attack(id).unwrap().retreating());
    let in_field = g.attack(id).unwrap().troops();
    let mut ticks = 0;
    while g.attack(id).unwrap().is_active() {
        let before_player = g.player(HUMAN).unwrap().clone();
        let before = before_player.troops();
        run_turn(&mut g, &[]);
        ticks += 1;
        assert!(ticks < 40);
        if !g.attack(id).unwrap().is_active() {
            let refund = g.player(HUMAN).unwrap().troops() - before;
            // The economy runs first that tick (its execution was added first), then the refund.
            let mut grown = before_player.clone();
            grown.add_troops(rules::troop_increase_rate(&config(), &before_player));
            let growth = grown.troops() - before;
            let survivors = (in_field * 0.75).floor() as i64;
            assert_eq!(refund, growth + survivors);
        }
    }
    assert!((19..=22).contains(&ticks), "retreat took {ticks} ticks");
}

#[test]
fn opposing_attacks_cancel_out() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 5, 5, 40, 50);
    give_rect(&mut g, 2, 41, 5, 90, 50);
    past_immunity(&mut g);
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: 2,
            target: Some(HUMAN),
            troops: Some(3_000.0),
        }],
    );
    let bot_attack = g.player(2).unwrap().outgoing_attacks()[0];
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: Some(2),
            troops: Some(10_000.0),
        }],
    );
    assert!(
        !g.attack(bot_attack).unwrap().is_active(),
        "smaller opposing attack removed"
    );
    let mine = g.player(HUMAN).unwrap().outgoing_attacks()[0];
    // 10 000 minus whatever the bot attack still had (it lost some conquering one tick).
    let t = g.attack(mine).unwrap().troops();
    assert!(t > 7_000.0 && t < 7_500.0, "troops {t}");
}

#[test]
fn attacks_on_the_same_target_merge() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 5, 5, 40, 50);
    give_rect(&mut g, 2, 41, 5, 90, 50);
    g.player_mut(2).unwrap().set_troops(5_000_000.0);
    past_immunity(&mut g);
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: Some(2),
            troops: Some(4_000.0),
        }],
    );
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: Some(2),
            troops: Some(3_000.0),
        }],
    );
    let out = g.player(HUMAN).unwrap().outgoing_attacks().to_vec();
    assert_eq!(out.len(), 1);
    assert!(g.attack(out[0]).unwrap().troops() > 6_000.0);
}

#[test]
fn small_defender_is_finished_off_and_loses_gold() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 5, 5, 60, 50);
    give_rect(&mut g, 2, 61, 20, 70, 28); // 90 tiles, under the 100-tile cutoff
    g.player_mut(2).unwrap().set_troops(10.0);
    g.player_mut(2).unwrap().add_gold(5_000);
    past_immunity(&mut g);
    let gold_before = g.player(HUMAN).unwrap().gold();
    run_turn(
        &mut g,
        &[Intent::Attack {
            player: HUMAN,
            target: Some(2),
            troops: Some(20_000.0),
        }],
    );
    for _ in 0..5 {
        run_turn(&mut g, &[]);
    }
    check_invariants(&g);
    assert_eq!(g.player(2).unwrap().num_tiles(), 0, "bot wiped out");
    assert!(!g.player(2).unwrap().is_alive());
    assert_eq!(g.player(2).unwrap().gold(), 0);
    assert!(g.player(HUMAN).unwrap().gold() >= gold_before + 5_000);
}

#[test]
fn encircled_pocket_is_annexed() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 5, 5, 60, 50);
    give_rect(&mut g, 2, 70, 5, 90, 50); // bot's main land, far away
                                         // Let the per-player cluster checks start, then cut a pocket out of human land for the bot.
                                         // (Upstream only re-checks a player after its land changed, which this does.)
    for _ in 0..25 {
        run_turn(&mut g, &[]);
    }
    give_rect(&mut g, 2, 30, 25, 33, 28); // a 16-tile pocket inside human land
    let pocket = g.map().tile(31, 26).unwrap();
    assert_eq!(g.owner(pocket), Some(2));
    for _ in 0..25 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(g.owner(pocket), Some(HUMAN), "pocket not annexed");
    assert_eq!(g.player(2).unwrap().num_tiles(), 21 * 46);
    check_invariants(&g);
}

#[test]
fn pocket_touching_unowned_land_is_not_annexed() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 5, 5, 60, 50);
    give_rect(&mut g, 2, 70, 5, 90, 50);
    for _ in 0..25 {
        run_turn(&mut g, &[]);
    }
    give_rect(&mut g, 2, 30, 25, 33, 28);
    let hole = g.map().tile(34, 26).unwrap();
    g.relinquish(hole); // an escape route of unowned land
    for _ in 0..25 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(g.owner(g.map().tile(31, 26).unwrap()), Some(2));
}

#[test]
fn owning_eighty_percent_of_land_wins() {
    let mut g = arena();
    let land = g.map().num_land_tiles();
    give_rect(&mut g, 2, 85, 1, 98, 58); // bot keeps a strip
    give_rect(&mut g, HUMAN, 1, 1, 84, 58);
    assert!(g.player(HUMAN).unwrap().num_tiles() as f64 > land as f64 * 0.8);
    for _ in 0..20 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(g.winner(), Some(HUMAN));
}

#[test]
fn no_winner_below_eighty_percent() {
    let mut g = arena();
    give_rect(&mut g, HUMAN, 1, 1, 70, 58);
    give_rect(&mut g, 2, 71, 1, 98, 58);
    for _ in 0..30 {
        run_turn(&mut g, &[]);
    }
    assert_eq!(g.winner(), None);
}

// ----- whole matches -----

/// Every player keeps launching attacks at random neighbours (or unowned land).
fn chaos_match(seed: i64, ticks: u32) -> Game {
    let mut g = new_match(config(), MatchSetup::bots_only(MapSize::Small, seed, 12)).unwrap();
    run_turn(&mut g, &[]);
    run_turn(&mut g, &[]);
    let map = g.map();
    let tile = (0..map.num_tiles())
        .step_by(101)
        .find(|&t| spawn_tiles(map, t, 4, true).is_some())
        .unwrap();
    run_turn(
        &mut g,
        &[Intent::Spawn {
            player: HUMAN,
            tile,
        }],
    );
    let mut rng = PseudoRandom::new(seed);
    for i in 0..ticks {
        let mut intents = Vec::new();
        if i % 10 == 0 {
            for p in g.alive_players() {
                let target = if rng.chance(3) {
                    None
                } else {
                    let all: Vec<PlayerId> = g
                        .alive_players()
                        .map(|q| q.id())
                        .filter(|&q| q != p.id())
                        .collect();
                    rng.index(all.len()).map(|k| all[k])
                };
                intents.push(Intent::Attack {
                    player: p.id(),
                    target,
                    troops: None,
                });
            }
        }
        run_turn(&mut g, &intents);
        if i % 50 == 0 {
            check_invariants(&g);
        }
    }
    check_invariants(&g);
    g
}

#[test]
fn chaotic_match_keeps_invariants_and_is_deterministic() {
    let a = chaos_match(21, 600);
    let b = chaos_match(21, 600);
    assert_eq!(a.state_hash(), b.state_hash());
    let owned: u32 = a.players().iter().map(|p| p.num_tiles()).sum();
    assert!(owned > 13 * 52, "players should have expanded, own {owned}");
}
