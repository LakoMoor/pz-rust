use pz_sim::{stress, Player};

#[test]
fn stress_levels_follow_the_thresholds() {
    assert_eq!([0.25, 0.26, 0.51, 0.76, 0.91].map(stress::moodle), [0, 1, 2, 3, 4]);
}

#[test]
fn calm_characters_relax_and_wounded_or_infected_ones_tense_up() {
    assert!(stress::tick(0.5, 0.02, false, false, false) < 0.5);
    assert_eq!(stress::tick(0.0, 0.02, false, false, false), 0.0);
    assert!(stress::tick(0.5, 0.02, true, false, false) > 0.5);
    let one = stress::tick(0.5, 0.02, true, false, false) - 0.5;
    let three = stress::tick(0.5, 0.02, true, true, true) - 0.5;
    assert!(three > one * 2.0);
    assert_eq!(stress::tick(0.99999, 100.0, true, true, true), 1.0);
}

#[test]
fn a_bitten_player_slowly_stresses_out() {
    let mut p = Player::new(0.0, 0.0);
    p.body.bite(9, 0.5, 0.5);
    for _ in 0..200_000 { p.tick_needs(0.02, false); }
    assert!(p.stress > 0.05 && p.stress <= 1.0, "{}", p.stress);
    let calm = { let mut q = Player::new(1.0, 1.0); for _ in 0..1000 { q.tick_needs(0.02, false); } q.stress };
    assert_eq!(calm, 0.0);
}
