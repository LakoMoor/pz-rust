use pz_sim::{body::Body, moodles::*};

#[test]
fn injured_follows_missing_health_in_steps() {
    assert_eq!(injured(100.0), 0);
    assert_eq!(injured(80.0), 0, "exactly 20 missing is not above the first threshold");
    assert_eq!(injured(79.0), 1);
    assert_eq!(injured(59.0), 2);
    assert_eq!(injured(39.0), 3);
    assert_eq!(injured(24.0), 4);
    assert_eq!(injured(0.0), 0, "the dead show no moodle");
}

#[test]
fn bleeding_counts_bleeding_parts_and_a_neck_wound_is_always_severe() {
    let mut b = Body::default();
    assert_eq!(bleeding(&b, 100.0), 0);
    b.start_bleeding(0, 5.0);
    assert_eq!(bleeding(&b, 100.0), 1);
    for i in 1..4 { b.start_bleeding(i, 5.0); }
    assert_eq!(bleeding(&b, 100.0), 4);
    for i in 4..8 { b.start_bleeding(i, 5.0); }
    assert_eq!(bleeding(&b, 100.0), 4, "capped at max");
    let mut n = Body::default();
    n.start_bleeding(9, 5.0);
    assert_eq!(bleeding(&n, 100.0), 4);
    assert_eq!(bleeding(&n, 0.0), 0);
}

#[test]
fn sick_rises_with_the_zombie_infection() {
    assert_eq!(sick(0.0, 0.0, 100.0), 0);
    assert_eq!(sick(25.0, 0.0, 100.0), 0, "exactly on a threshold does not count");
    assert_eq!(sick(26.0, 0.0, 100.0), 1);
    assert_eq!(sick(51.0, 0.0, 100.0), 2);
    assert_eq!(sick(76.0, 0.0, 100.0), 3);
    assert_eq!(sick(91.0, 0.0, 100.0), 4);
    // The separate sickness stat adds to it.
    assert_eq!(sick(30.0, 0.3, 100.0), 2);
    assert_eq!(sick(95.0, 0.0, 0.0), 0);
}

#[test]
fn an_infected_player_gets_sicker_on_the_way_to_death() {
    use pz_sim::Player;
    let mut p = Player::new(0.0, 0.0);
    p.infected = true;
    let mut seen = [false; 5];
    for step in 0..=40 {
        let now = step as f32 * 0.1; // mortality 4, roll 0 -> 3 hours
        p.tick_infection(now, 4, 0.0);
        seen[sick(p.infection_pct, 0.0, if p.health > 0.0 { 100.0 } else { 0.0 }) as usize] = true;
    }
    assert!(seen[0] && seen[1] && seen[2] && seen[3], "{seen:?}");
}

#[test]
fn pain_stat_creeps_up_with_wounds_and_levels_follow_thresholds() {
    use pz_sim::Player;
    assert_eq!(pain(10.0, 100.0), 0);
    assert_eq!(pain(11.0, 100.0), 1);
    assert_eq!(pain(21.0, 100.0), 2);
    assert_eq!(pain(51.0, 100.0), 3);
    assert_eq!(pain(76.0, 100.0), 4);
    assert_eq!(pain(90.0, 0.0), 0);
    let mut p = Player::new(0.0, 0.0);
    p.body.parts[8].cut_time = 20.0; // head cut: 50 pain * 0.8 = 40
    p.body.parts[7].deep_wound_time = 10.0; // torso: 37 * 0.78
    let target = p.body.pain_target();
    assert!((target - (50.0 * 0.8 + 37.0 * 0.78)).abs() < 1e-3, "{target}");
    p.tick_body();
    assert!(p.pain_stat > 0.0 && p.pain_stat < target / 100.0, "pain creeps up: {}", p.pain_stat);
    for _ in 0..3000 { p.tick_body(); }
    assert!(p.pain_stat > 0.9 * target.min(60.0), "after ~100 s it has nearly caught up: {} vs {target}", p.pain_stat);
    // When the wounds heal the stat drops immediately.
    p.body.parts[8].cut_time = 0.0;
    p.body.parts[7].deep_wound_time = 0.0;
    p.tick_body();
    // It snaps down to what is left (a wound may have started to fester meanwhile), not back up slowly.
    assert_eq!(p.pain_stat, p.body.pain_target());
    assert!(p.pain_stat < 0.5, "{}", p.pain_stat);
}

#[test]
fn a_severe_bleeding_moodle_drains_health_on_top_of_the_wound() {
    use pz_sim::Player;
    let mut neck = Player::new(0.0, 0.0);
    neck.body.start_bleeding(9, 10.0); // neck: always max bleeding moodle
    let mut leg = Player::new(0.0, 0.0);
    leg.body.start_bleeding(15, 10.0); // foot: one bleeding part, no severe moodle
    // Compare health lost per unit of bleeding drain by running both and subtracting the plain wound loss.
    for _ in 0..300 { neck.tick_body(); leg.tick_body(); }
    let severe_loss = 100.0 - neck.health;
    assert!(severe_loss > 300.0 * 0.0165 * 0.9, "the moodle alone costs ~0.0165 per tick: {severe_loss}");
    assert!(neck.health < leg.health);
    // Dead characters stop taking moodle damage.
    let mut dead = Player::new(0.0, 0.0);
    dead.body.start_bleeding(9, 10.0);
    for p in dead.body.parts.iter_mut() { p.health = 0.0; }
    dead.health = 0.0;
    dead.tick_body();
    assert_eq!(dead.health, 0.0);
}

#[test]
fn temperature_moodles_follow_the_core_temperature() {
    use pz_sim::moodles::{hyperthermia, hypothermia};
    assert_eq!((hypothermia(37.0), hyperthermia(37.0)), (0, 0));
    assert_eq!([36.4, 34.9, 29.9, 24.9].map(hypothermia), [1, 2, 3, 4]);
    assert_eq!([37.6, 39.1, 40.1, 41.1].map(hyperthermia), [1, 2, 3, 4]);
    assert_eq!(hypothermia(36.5), 0);
    assert_eq!(hyperthermia(37.5), 0);
}
