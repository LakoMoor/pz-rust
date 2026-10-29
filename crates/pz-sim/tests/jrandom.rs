use pz_sim::jrandom::JavaRandom;

#[test]
fn matches_the_well_known_java_util_random_sequence() {
    // new Random(42): nextInt() -> -1170105035, 234785527 (documented JDK behaviour).
    let mut r = JavaRandom::new(42);
    assert_eq!(r.next_int(), -1170105035);
    assert_eq!(r.next_int(), 234785527);
    // new Random(0).nextInt() = -1155484576; nextInt(10) on seed 42 starts 0, 3.
    assert_eq!(JavaRandom::new(0).next_int(), -1155484576);
    let mut r = JavaRandom::new(42);
    assert_eq!((r.next_int_bound(10), r.next_int_bound(10)), (0, 3));
    let mut r = JavaRandom::new(7);
    for _ in 0..1000 { let f = r.next_float(); assert!((0.0..1.0).contains(&f)); let d = r.next_double(); assert!((0.0..1.0).contains(&d)); }
}
