use pz_sim::infection::*;

#[test]
fn mortality_setting_picks_the_time_to_death() {
    assert_eq!(mortality_hours(1, 0.5, 1.0), Some(0.0));
    assert_eq!(mortality_hours(7, 0.5, 1.0), None);
    // Setting 4: 3-12 hours, scaled by traits.
    assert_eq!(mortality_hours(4, 0.0, 1.0), Some(3.0));
    assert_eq!(mortality_hours(4, 1.0, 1.0), Some(12.0));
    assert_eq!(mortality_hours(4, 0.0, 0.75), Some(2.25));
    // Setting 5: 2-3 days; setting 6: 1-2 weeks; setting 2: seconds.
    assert_eq!(mortality_hours(5, 0.0, 1.0), Some(48.0));
    assert_eq!(mortality_hours(6, 1.0, 1.0), Some(336.0));
    assert!((mortality_hours(2, 1.0, 1.0).unwrap() - 30.0 / 3600.0).abs() < 1e-9);
}

#[test]
fn health_is_capped_by_a_curve_that_collapses_near_the_end() {
    let inf = Infection { start: 100.0, duration: 10.0 };
    // Just infected: sickness 100 -> no cap on a healthy character.
    assert_eq!(inf.apply(100.0, 100.0), 100.0);
    // Halfway: p^4 = 1/16 -> cap 93.75.
    assert!((inf.apply(105.0, 100.0) - 93.75).abs() < 1e-4);
    // Already below the cap: untouched.
    assert_eq!(inf.apply(105.0, 50.0), 50.0);
    // 90% in: cap = (1 - 0.6561) * 100.
    assert!((inf.apply(109.0, 100.0) - 34.39).abs() < 0.01);
    // At the end the character takes lethal damage regardless of health.
    assert!(inf.apply(110.0, 100.0) <= -10.0 + 1e-4);
    assert_eq!(inf.progress(90.0), 0.0);
    assert_eq!(Infection { start: 0.0, duration: 0.0 }.progress(0.0), 1.0);
}

#[test]
fn a_bitten_player_sickens_and_dies_on_schedule() {
    use pz_sim::Player;
    let mut p = Player::new(0.0, 0.0);
    // Healthy and not infected: nothing happens.
    assert!(!p.tick_infection(0.0, 4, 0.0) && p.health == 100.0);
    p.infected = true;
    // Mortality 4 with roll 0 -> 3 hours to live.
    let mut died_at = None;
    for step in 0..=40 {
        let now = 10.0 + step as f32 * 0.1;
        if p.tick_infection(now, 4, 0.0) { died_at = Some(now); break; }
    }
    let t = died_at.expect("should die");
    assert!((t - 13.0).abs() < 0.11, "died at {t}");
    assert_eq!(p.health, 0.0);
    // Mortality 7: infected forever, never harmed.
    let mut q = Player::new(0.0, 0.0);
    q.infected = true;
    for h in 0..1000 { assert!(!q.tick_infection(h as f32, 7, 0.3)); }
    assert_eq!(q.health, 100.0);
}
