use pz_sim::clothing::*;
use pz_sim::Player;

#[test]
fn garments_cover_the_expected_parts() {
    assert_eq!(covered_parts("Head"), vec![8]);
    assert_eq!(covered_parts("Shoes"), vec![15, 16]);
    assert_eq!(covered_parts("Jacket").len(), 2 + 2 + 2 + 1, "torso, upper arms, forearms, neck");
    assert!(covered_parts("Trousers").contains(&14) && !covered_parts("Trousers").contains(&6));
    assert!(covered_parts("Nonsense").is_empty());
}

#[test]
fn insulation_stacks_and_wet_or_worn_clothing_insulates_less() {
    let coat = Garment::new("Jacket", 0.8, 0.7);
    let (dry, wind) = node_values(&[coat.clone()]);
    assert!(dry[6] > 1.5 && wind[6] > 0.7);
    assert_eq!(dry[16], 0.0, "feet are not covered");
    let mut wet = coat.clone(); wet.wetness = 100.0;
    assert!(node_values(&[wet]).0[6] < dry[6] * 0.5);
    let mut worn_out = coat.clone(); worn_out.condition = 0.0;
    assert!((node_values(&[worn_out]).0[6] - (true_insulation(0.8) * 0.5 + 0.05)).abs() < 1e-5);
    let two = node_values(&[coat.clone(), coat]).0[6];
    assert!(two > dry[6] * 1.9);
}

#[test]
fn a_coat_keeps_a_player_warmer_in_the_cold() {
    let run = |worn: Vec<Garment>| {
        let mut p = Player::new(0.0, 0.0);
        p.worn = worn;
        for _ in 0..40_000 { p.tick_thermo(-5.0, -5.0, 0.5, false, 1.0, false); }
        p.thermo.core_celcius()
    };
    let bare = run(vec![]);
    let dressed = run(vec![Garment::new("Jacket", 0.9, 0.9), Garment::new("Trousers", 0.7, 0.6), Garment::new("Head", 0.9, 0.9)]);
    assert!(dressed > bare + 0.3, "dressed {dressed} bare {bare}");
}
