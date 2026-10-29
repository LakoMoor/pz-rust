use pz_sim::*;

#[test]
fn vision_radius_follows_the_decompiled_formula() {
    assert_eq!(Weather::default().vision_radius(), 20.0);
    // Darkness 5 vs rain+fog 0: 20 - 5 = 15.
    assert_eq!(Weather { rain: 0.0, fog: 0.0, light: 0.0 }.vision_radius(), 15.0);
    // The worse of the two penalties applies, not their sum: fog 0.5 -> 3.5, rain 1 -> 2.5, total 6.
    let w = Weather { rain: 1.0, fog: 0.5, light: 0.0 };
    assert_eq!(w.vision_radius(), 14.0);
    // Heavy fog clamps at 10.
    assert_eq!(Weather { rain: 1.0, fog: 1.0, light: 1.0 }.vision_radius(), 10.5);
    assert_eq!(Weather { rain: 5.0, fog: 5.0, light: 1.0 }.vision_radius(), 10.0);
}
