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
