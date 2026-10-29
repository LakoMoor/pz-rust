use pz_sim::climate::roll_fog;

#[test]
fn fog_roll_is_deterministic_and_about_one_day_in_five() {
    assert_eq!(roll_fog(1993, 6, 9, 100, 250, 7.0, 15.0), roll_fog(1993, 6, 9, 100, 250, 7.0, 15.0));
    let mut foggy = 0;
    for d in 0..2000 {
        let (month, day) = ((d / 28) % 12, d % 28 + 1);
        if let Some(f) = roll_fog(1993 + d / 336, month, day, 17, 91, 7.0, 12.0) {
            foggy += 1;
            assert!((0.0..=1.0).contains(&f.strength));
            assert!(f.duration >= 4.0 && f.duration <= 16.0, "{f:?}");
        }
    }
    let share = foggy as f32 / 2000.0;
    assert!((0.15..0.25).contains(&share), "fog share {share}");
}

#[test]
fn colder_days_have_longer_fog_and_it_is_capped_by_the_day_length() {
    // Same seed (same date, offsets), only the temperature differs. Find a foggy day.
    let (y, m, d) = (0..400).map(|i| (1993, i % 12, i % 28 + 1)).find(|&(y, m, d)| roll_fog(y, m, d, 5, 9, 7.0, 15.0).is_some()).unwrap();
    let cold = roll_fog(y, m, d, 5, 9, 7.0, -5.0).unwrap();
    let warm = roll_fog(y, m, d, 5, 9, 7.0, 25.0).unwrap();
    assert_eq!(cold.strength, warm.strength);
    assert!(cold.duration > warm.duration);
    let late_dawn = roll_fog(y, m, d, 5, 9, 22.0, -5.0).unwrap();
    assert_eq!(late_dawn.duration, 24.0 - 22.0 - 1.0);
}
