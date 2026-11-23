//! Milestone P12: team matches.

use laukofront::config::{GameConfig, MapSize, GAME_CONFIG_PATH};
use laukofront::engine::{new_match, run_turn, Game, MatchSetup, PlayerType, HUMAN};

fn team_match(teams: u8, seed: i64) -> Game {
    let mut s = MatchSetup::bots_only(MapSize::Small, seed, 8);
    s.nations = 6;
    s.teams = teams;
    new_match(GameConfig::load(GAME_CONFIG_PATH).unwrap(), s).unwrap()
}

#[test]
fn players_are_shared_out_and_bots_belong_to_no_team() {
    for teams in [2u8, 3, 4] {
        let g = team_match(teams, 1);
        assert_eq!(g.num_teams(), teams);
        let mut sizes = vec![0; teams as usize];
        for p in g.players() {
            match p.kind() {
                PlayerType::Bot => assert_eq!(p.team(), None),
                _ => sizes[p.team().unwrap() as usize] += 1,
            }
        }
        let (lo, hi) = (*sizes.iter().min().unwrap(), *sizes.iter().max().unwrap());
        assert!(hi - lo <= 1, "{teams} teams unbalanced: {sizes:?}");
    }
    assert_eq!(team_match(0, 1).num_teams(), 0, "free for all");
}

#[test]
fn teammates_are_friendly_and_never_fight() {
    let mut g = team_match(2, 2);
    let mate = g
        .players()
        .iter()
        .find(|p| p.id() != HUMAN && p.team() == Some(0))
        .unwrap()
        .id();
    assert!(g.is_friendly(HUMAN, mate));
    assert!(!g.can_attack_player(HUMAN, mate));
    let bot = g
        .players()
        .iter()
        .find(|p| p.kind() == PlayerType::Bot)
        .unwrap()
        .id();
    assert!(!g.is_friendly(HUMAN, bot));
    // Over a whole match no attack ever targets a teammate.
    while g.winner().is_none() && g.ticks() < 20_000 {
        run_turn(&mut g, &[]);
        for p in g.alive_players() {
            for &a in p.outgoing_attacks() {
                if let Some(t) = g.attack(a).and_then(|a| a.target()) {
                    assert!(
                        !g.same_team(p.id(), t),
                        "{} attacked teammate {}",
                        p.name(),
                        g.player(t).unwrap().name()
                    );
                }
            }
        }
    }
    let t = g.winner_team().expect("a team wins");
    let w = g.winner().unwrap();
    assert_eq!(g.player(w).unwrap().team(), Some(t));
}
