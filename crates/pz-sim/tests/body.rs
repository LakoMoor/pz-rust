use pz_sim::body::*;

#[test]
fn overall_health_weighs_parts_by_their_damage_modifier() {
    let mut b = Body::default();
    assert_eq!(b.overall_health(), 100.0);
    b.parts[8].health = 50.0; // head: modifier 0.6 -> 30 damage
    assert!((b.overall_health() - 70.0).abs() < 1e-4);
    // Even a wrecked body bottoms out at zero.
    for p in b.parts.iter_mut() { p.health = 0.0; }
    assert_eq!(b.overall_health(), 0.0);
}

#[test]
fn general_damage_is_spread_across_all_parts() {
    let mut b = Body::default();
    b.reduce_general_health(17.0);
    // Each part loses 1 / modifier of its health: the hands (0.1) lose 10, the neck (0.7) about 1.43.
    assert!((b.parts[0].health - 90.0).abs() < 1e-3);
    assert!((b.parts[9].health - (100.0 - 1.0 / 0.7)).abs() < 1e-3);
    // Dividing by the modifier cancels the weighting, so overall health drops by exactly the amount taken.
    assert!((b.overall_health() - 83.0).abs() < 1e-3);
}

#[test]
fn bleeding_drains_health_until_it_stops_and_a_bandage_staunches_it() {
    let mut open = Body::default();
    open.start_bleeding(8, 10.0); // head: 10 * 1.0
    let mut wrapped = open.clone();
    wrapped.parts[8].bandaged = true;
    wrapped.parts[8].bandage_life = 100.0;
    for _ in 0..3000 { open.update(1.0); wrapped.update(1.0); }
    assert!(open.overall_health() < 100.0, "an open wound bleeds: {}", open.overall_health());
    assert!(open.parts[8].bleeding_time < 10.0 && open.parts[8].bleeding_time > 0.0);
    assert_eq!(wrapped.overall_health(), 100.0, "a bandaged wound does no bleed damage");
    assert!(wrapped.parts[8].bleeding_time < open.parts[8].bleeding_time, "but a bandage clots faster");
    assert!(wrapped.parts[8].bandage_life < 100.0, "and wears out");
    // Fully clotted wounds reach exactly zero and stop counting.
    wrapped.parts[8].bleeding_time = 1.0e-5;
    wrapped.update(1.0);
    assert_eq!(wrapped.num_bleeding(), 0);
}

#[test]
fn wounds_hurt_their_own_part_unless_bandaged() {
    let mut b = Body::default();
    b.parts[6].scratch_time = 5.0;
    b.parts[7].bite_time = 5.0;
    b.parts[7].bandaged = true;
    b.parts[7].bandage_life = 10.0;
    b.parts[10].deep_wound_time = 5.0;
    b.update(1.0);
    assert!((b.parts[6].health - (100.0 - 0.9375 * 0.0057142857)).abs() < 1e-4);
    assert_eq!(b.parts[7].health, 100.0, "a bandaged bite stops damaging the part");
    assert!(b.parts[7].bite_time < 5.0 && b.parts[7].bandage_life < 10.0);
    assert!((b.parts[10].health - (100.0 - 3.125 * 0.0057142857)).abs() < 1e-4);
    // Stitching stops deep-wound damage.
    b.parts[10].stitched = true;
    let h = b.parts[10].health;
    b.update(1.0);
    assert_eq!(b.parts[10].health, h);
}

#[test]
fn a_zombie_scratch_leaves_a_bleeding_wound_that_keeps_costing_health() {
    use pz_sim::*;
    let mut p = Player::new(0.0, 0.0);
    p.body.scratch(8, 0.5, 0.5); // head: scratch 11 -> bleeds 0.45 * 11 * 1.0
    assert!((p.body.parts[8].bleeding_time - 4.95).abs() < 1e-3);
    let start = p.health;
    for _ in 0..600 { p.tick_body(); }
    assert!(p.health < start, "bleeding costs health: {}", p.health);
    // Scratches and cuts also hurt directly; a bite lasts far longer.
    let mut q = Player::new(0.0, 0.0);
    q.body.bite(7, 0.0, 0.0);
    assert_eq!(q.body.parts[7].bite_time, 50.0);
    assert!((q.body.parts[7].bleeding_time - 7.5 * 0.9).abs() < 1e-4);
}

#[test]
fn zombie_hits_wound_the_player() {
    use pz_sim::*;
    use pz_world::World;
    let mut w = World::new(std::path::PathBuf::from("."));
    let mut p = Player::new(0.0, 0.0);
    let mut z = Zombie::new(0.5, 0.0, 0);
    for _ in 0..600 { z.tick(&mut p, &mut w); if p.health <= 0.0 { break; } }
    assert!(p.health < 100.0);
    assert!(p.body.parts.iter().any(|b| b.scratch_time > 0.0 || b.cut_time > 0.0 || b.bite_time > 0.0), "a landed hit should leave a wound");
}

#[test]
fn a_bandaged_scratch_heals_long_before_an_open_one() {
    let mut open = Body::default();
    open.scratch(6, 0.0, 0.0); // 7 time units
    let mut wrapped = open.clone();
    wrapped.apply_bandage(6, 100.0);
    assert!(wrapped.parts[6].bandaged && !open.parts[6].bandaged);
    let mut ticks = 0;
    while wrapped.parts[6].scratch_time > 0.0 && ticks < 200_000 { wrapped.update(1.0); ticks += 1; }
    assert_eq!(wrapped.parts[6].scratch_time, 0.0, "healed");
    assert_eq!(wrapped.parts[6].bleeding_time, 0.0, "and the bleeding ends with it");
    // ~7 / 1.5e-4 = 46,667 ticks; the same scratch left open has barely moved.
    assert!((46_000..48_000).contains(&ticks), "{ticks}");
    for _ in 0..ticks { open.update(1.0); }
    assert!(open.parts[6].scratch_time > 6.0, "{}", open.parts[6].scratch_time);
    // The bandage wore down but never goes negative, and then counts as dirty.
    assert!(wrapped.parts[6].bandage_life < 100.0);
    wrapped.parts[6].bandage_life = 0.0;
    assert!(wrapped.bandage_is_dirty(6));
    wrapped.remove_bandage(6);
    assert!(!wrapped.bandage_is_dirty(6));
}

#[test]
fn deep_wounds_only_close_under_a_bandage() {
    let mut b = Body::default();
    b.parts[7].deep_wound_time = 3.001;
    for _ in 0..5_000 { b.update(1.0); }
    assert_eq!(b.parts[7].deep_wound_time, 3.0, "open deep wound is stuck at 3");
    b.apply_bandage(7, 1_000.0);
    for _ in 0..200_000 { b.update(1.0); }
    assert!(b.parts[7].deep_wound_time < 3.0, "bandaged it keeps healing: {}", b.parts[7].deep_wound_time);
}

#[test]
fn pain_follows_the_wounds_and_eases_under_a_bandage() {
    let mut b = Body::default();
    assert_eq!(b.parts[8].pain(), 0.0);
    b.parts[8].scratch_time = 10.0; // 17
    b.parts[8].cut_time = 4.0;      // 10
    b.parts[8].deep_wound_time = 2.0; // 7.4
    assert!((b.parts[8].pain() - 34.4).abs() < 1e-4);
    b.apply_bandage(8, 10.0);
    assert!((b.parts[8].pain() - 34.4 / 1.5).abs() < 1e-4);
    // A bite hurts a flat 50, or 30 once wrapped (before the bandage's own 1/1.5).
    b.parts[3].bite_time = 60.0;
    assert_eq!(b.parts[3].pain(), 50.0);
    b.apply_bandage(3, 10.0);
    assert!((b.parts[3].pain() - 20.0).abs() < 1e-4);
}

#[test]
fn stitches_close_a_deep_wound_hurt_for_a_while_and_reopen_if_pulled_early() {
    let mut b = Body::default();
    b.parts[7].deep_wound_time = 20.0;
    b.parts[7].bleeding_time = 15.0;
    b.stitch(7, 3.0, 20.0);
    let p = b.parts[7];
    assert!(p.stitched && p.deep_wound_time == 0.0 && p.bleeding_time == 0.0);
    // Stitch pain (35 - 3) plus the procedure's lingering 20.
    assert!((p.pain() - 52.0).abs() < 1e-4);
    // The lingering pain fades by 0.005 per tick; the stitch itself matures slowly.
    for _ in 0..1000 { b.update(1.0); }
    assert!((b.parts[7].additional_pain - 15.0).abs() < 0.01);
    assert!(b.parts[7].stitch_time > 3.1);
    // Pulling them out early (stitch time 3 < 40) reopens a deep wound that bleeds.
    b.remove_stitches(7, 0.5, 0.5, 5.0);
    assert!(!b.parts[7].stitched && b.parts[7].deep_wound_time > 0.0 && b.parts[7].bleeding_time > 0.0);
    // Matured stitches (over 40) leave only a scratch.
    let mut c = Body::default();
    c.stitch(2, 45.0, 0.0);
    c.remove_stitches(2, 0.0, 0.0, 0.0);
    assert!(c.parts[2].scratch_time >= 2.0 && c.parts[2].deep_wound_time == 0.0);
}

#[test]
fn a_fracture_hurts_and_damages_until_splinted_which_also_heals_faster() {
    let mut open = Body::default();
    open.fracture(13, 60.0); // lower leg
    assert_eq!(open.parts[13].pain(), 60.0);
    let mut splinted = open.clone();
    splinted.apply_splint(13, 2.0);
    assert_eq!(splinted.parts[13].pain(), 30.0, "a set bone hurts half as much");
    for _ in 0..20_000 { open.update(1.0); splinted.update(1.0); }
    assert!(open.parts[13].health < 100.0, "an unsplinted fracture keeps injuring the leg");
    assert_eq!(splinted.parts[13].health, 100.0, "a splint stops the damage");
    // Open heals at 5e-6 per tick, splinted at 5e-5 * factor = 1e-4. The unsplinted increments sit near the f32
    // resolution at 60 (as in the original, which also uses floats), so only the order of magnitude is stable.
    let (a, b) = (60.0 - open.parts[13].fracture_time, 60.0 - splinted.parts[13].fracture_time);
    assert!(a > 0.0 && b > 10.0 * a, "{a} {b}");
    splinted.remove_splint(13);
    assert!(!splinted.parts[13].splint && splinted.parts[13].splint_factor == 0.0);
    // It heals completely and never goes negative.
    splinted.parts[13].fracture_time = 1.0e-9;
    splinted.update(1.0);
    assert_eq!(splinted.parts[13].fracture_time, 0.0);
}
