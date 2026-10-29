use pz_sim::melee_crit_chance;

#[test]
fn crit_chance_matches_the_original_formula() {
    // SpadeHead has CriticalChance = 15 in the item scripts.
    assert_eq!(melee_crit_chance(15.0, false, 2.0, 0), 15);
    assert_eq!(melee_crit_chance(15.0, false, 1.0, 0), 12); // 15 - 3 (weak swing)
    assert_eq!(melee_crit_chance(15.0, true, 2.0, 0), 10); // 15 - 5
    assert_eq!(melee_crit_chance(15.0, false, 2.0, 5), 30); // +3 per skill level
    assert_eq!(melee_crit_chance(0.0, false, 2.0, 0), 10, "clamped to 10..90");
    assert_eq!(melee_crit_chance(95.0, false, 2.0, 10), 90);
}
