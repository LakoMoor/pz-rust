use pz_sim::simplex::noise;

#[test]
fn noise_is_zero_at_the_origin_bounded_and_deterministic() {
    assert_eq!(noise(0.0, 0.0), 0.0);
    let mut max = 0.0f64;
    for i in 0..4000 {
        let (x, y) = (i as f64 * 0.137 - 200.0, (i * 7 % 311) as f64 * 0.093 - 14.0);
        let v = noise(x, y);
        assert_eq!(v, noise(x, y));
        assert!(v.abs() <= 1.0, "{v} at {x},{y}");
        max = max.max(v.abs());
    }
    assert!(max > 0.7, "noise should use most of its range, max {max}");
}

#[test]
fn noise_is_continuous_and_handles_negative_coordinates() {
    for i in 0..2000 {
        let (x, y) = (i as f64 * 0.31 - 300.0, 5.0 - i as f64 * 0.17);
        let (a, b) = (noise(x, y), noise(x + 1e-6, y));
        assert!((a - b).abs() < 1e-4, "jump at {x},{y}: {a} vs {b}");
    }
}
