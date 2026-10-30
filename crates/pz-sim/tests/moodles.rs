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
