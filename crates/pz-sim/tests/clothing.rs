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

#[test]
fn rain_soaks_the_outer_layer_first_and_clothes_dry_out() {
    use pz_sim::wetness::Weather;
    let mut p = Player::new(0.0, 0.0);
    p.worn = vec![Garment::new("Shirt", 0.3, 0.1), Garment::new("Jacket", 0.8, 0.6)];
    let rain = Weather { outside: true, raining: true, rain_intensity: 1.0, air_temperature: 10.0 };
    for _ in 0..2000 { p.tick_wetness(rain); }
    assert!(p.worn[1].wetness > 20.0, "coat {}", p.worn[1].wetness);
    assert!(p.worn[0].wetness < p.worn[1].wetness, "shirt {}", p.worn[0].wetness);
    let wet_ins = node_values(&p.worn).0[6];
    assert!(wet_ins < node_values(&[Garment::new("Shirt", 0.3, 0.1), Garment::new("Jacket", 0.8, 0.6)]).0[6]);
    let dry_weather = Weather { outside: false, raining: false, rain_intensity: 0.0, air_temperature: 25.0 };
    let before = p.worn[1].wetness;
    for _ in 0..5000 { p.tick_wetness(dry_weather); }
    assert!(p.worn[1].wetness < before);
}

#[test]
fn skin_under_clothing_stays_dry_until_the_inner_layer_is_soaked() {
    use pz_sim::wetness::Weather;
    let rain = Weather { outside: true, raining: true, rain_intensity: 1.0, air_temperature: 10.0 };
    let mut p = Player::new(0.0, 0.0);
    p.worn = vec![Garment::new("Shirt", 0.3, 0.1)];
    let mut dry_at = None;
    for t in 0..200_000 {
        p.tick_wetness(rain);
        // Every part is blended toward the body average, so covered skin is never exactly dry, but a dry shirt must
        // not wet it directly: the exposed part ahead of it stays the wettest.
        if p.worn[0].wetness < 50.0 { assert!(p.wetness[6] <= p.wetness[0] + 1e-4, "tick {t}"); }
        else if dry_at.is_none() { dry_at = Some(t); }
    }
    assert!(dry_at.is_some() && p.wetness[6] > 0.0, "shirt {} skin {}", p.worn[0].wetness, p.wetness[6]);
    // Uncovered parts (feet) got wet right away.
    let mut q = Player::new(0.0, 0.0);
    q.worn = vec![Garment::new("Shirt", 0.3, 0.1)];
    for _ in 0..500 { q.tick_wetness(rain); }
    assert!(q.wetness[16] > 0.0);
}
