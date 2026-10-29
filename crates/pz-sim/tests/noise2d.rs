use pz_sim::noise2d::Noise2D;

/// The three layers `ErosionSeason.init` adds.
fn season(a: i32, b: i32, c: i32) -> Noise2D {
    let mut n = Noise2D::new();
    n.add_layer(a, 8.0, 2.0);
    n.add_layer(b, 6.0, 4.0);
    n.add_layer(c, 4.0, 6.0);
    n
}

#[test]
fn layered_noise_is_normalised_deterministic_and_seed_dependent() {
    let (n, m) = (season(64, 128, 255), season(1, 2, 3));
    let mut differs = 0;
    let (mut lo, mut hi) = (1.0f32, 0.0f32);
    for i in 0..3000 {
        let x = i as f32 * 0.37 - 400.0;
        let v = n.layered_noise(x, 0.0);
        assert_eq!(v, n.layered_noise(x, 0.0));
        assert!((0.0..=1.0).contains(&v), "{v} at {x}");
        lo = lo.min(v); hi = hi.max(v);
        if (v - m.layered_noise(x, 0.0)).abs() > 1e-4 { differs += 1; }
    }
    assert!(differs > 2000, "seeds should change the field ({differs})");
    assert!(hi - lo > 0.4, "range {lo}..{hi}");
}

#[test]
fn nearby_inputs_give_nearby_values_and_reset_clears_layers() {
    let n = season(64, 128, 255);
    for i in 0..1000 {
        let x = i as f32 * 0.05;
        // Smooth: a tiny step moves the value only a little (no lattice jumps).
        assert!((n.layered_noise(x, 0.0) - n.layered_noise(x + 0.005, 0.0)).abs() < 0.1);
    }
    let mut e = season(1, 1, 1);
    e.reset();
    assert!(e.layered_noise(1.0, 1.0).is_nan(), "no layers -> 0/0");
}
