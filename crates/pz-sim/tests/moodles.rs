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
