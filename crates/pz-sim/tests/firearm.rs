use pz_sim::firearm::Firearm;

/// Base.Pistol: HitChance 50, MinSightRange 2, MaxSightRange 6.
const PISTOL: Firearm = Firearm { hit_chance: 50.0, min_sight_range: 2.0, max_sight_range: 6.0 };

#[test]
fn hit_chance_matches_hand_computed_values_from_the_decompiled_formula() {
    // Centre of the sight band (dist 4): sights bonus 15 beats the sightless -7.7 -> 65.
    assert_eq!(PISTOL.hit_chance(4.0, false, 0.0, 0.0), 65);
    // Far beyond the band (dist 12): -(6 * (4 + 6 * 0.3)) = -34.8 -> 15.2 -> 15.
    assert_eq!(PISTOL.hit_chance(12.0, false, 0.0, 0.0), 15);
    // Hopeless range clamps to the 5% floor.
    assert_eq!(PISTOL.hit_chance(40.0, false, 0.0, 0.0), 5);
    // The starting chance is capped at 95 before any bonus.
    let sniper = Firearm { hit_chance: 120.0, ..PISTOL };
    assert_eq!(sniper.hit_chance(4.0, false, 0.0, 0.0), 100);
}

#[test]
fn moving_and_unfinished_aiming_cost_accuracy() {
    let still = PISTOL.hit_chance(4.0, false, 0.0, 0.0);
    assert!(PISTOL.hit_chance(4.0, false, 0.0, 20.0) < still);
    assert!(PISTOL.hit_chance(4.0, false, 10.0, 0.0) < still);
    // Move penalty at dist 4 is 20 * 0.4 = 8 -> 57.
    assert_eq!(PISTOL.hit_chance(4.0, false, 0.0, 20.0), 57);
}

#[test]
fn point_blank_is_easy_even_for_a_bad_gun() {
    assert!(PISTOL.hit_chance(1.0, false, 0.0, 0.0) >= 90);
    let prone_bonus = PISTOL.hit_chance(2.5, true, 0.0, 0.0) >= PISTOL.hit_chance(2.5, false, 0.0, 0.0);
    assert!(prone_bonus);
}
