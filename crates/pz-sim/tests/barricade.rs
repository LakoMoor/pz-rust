use pz_sim::*;
use pz_world::{flag, Barricade, World, DOOR_HEALTH, METAL_HEALTH, PLANK_HEALTH};

fn world() -> Option<World> {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let media = std::path::PathBuf::from(dir).join("media");
    if !media.is_dir() { return None; }
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    Some(w)
}

fn door(w: &mut World) -> Option<(i32, i32)> {
    for y in 1500..1800 {
        for x in 12500..12800 {
            if w.flags(x, y, 0) & flag::DOOR_N == 0 || w.door_open(x, y, 0) { continue; }
            let free = |w: &mut World, x, y| w.has_floor(x, y, 0) && w.flags(x, y, 0) & flag::SOLID == 0;
            if free(w, x, y) && free(w, x, y - 1) && free(w, x, y + 1) && free(w, x, y - 2) { return Some((x, y)); }
        }
    }
    None
}

#[test]
fn damage_goes_to_metal_then_bars_then_the_top_plank() {
    let mut b = Barricade { planks: [1000, 600, 0, 0], metal_bar: 0, metal: 0 };
    assert_eq!((b.num_planks(), b.health()), (2, 800));
    b.damage(700); // the second-nailed plank is the one hit
    assert_eq!(b.planks, [1000, 0, 0, 0]);
    b.damage(1000);
    assert!(b.is_destroyed());
    let mut m = Barricade { planks: [1000; 4], metal_bar: 3000, metal: 5000 };
    m.damage(5000);
    assert_eq!((m.metal, m.metal_bar, m.planks), (0, 3000, [1000; 4]));
}

#[test]
fn planks_and_metal_exclude_each_other() {
    let Some(mut w) = world() else { return };
    let (x, y) = door(&mut w).expect("a door near Muldraugh");
    let e = (x, y, 0, true);
    for _ in 0..4 { assert!(w.add_plank(e, PLANK_HEALTH)); }
    assert!(!w.add_plank(e, PLANK_HEALTH), "four planks is the limit");
    assert!(!w.add_metal(e, false), "no metal over planks");
    assert_eq!(w.toggle_door(x, y, 0), None, "a barricaded door will not open");
    let e2 = (x + 1, y, 0, true);
    assert!(w.add_metal(e2, false) && !w.add_plank(e2, 1));
    assert_eq!(w.barricade(e2).unwrap().metal, METAL_HEALTH);
}

#[test]
fn a_barricade_holds_the_crowd_that_would_break_the_bare_door() {
    let Some(mut w) = world() else { return };
    let (x, y) = door(&mut w).expect("a door near Muldraugh");
    for _ in 0..4 { w.add_plank((x, y, 0, true), PLANK_HEALTH); }
    let mut zs: Vec<Zombie> = (0..3).map(|i| Zombie::new(x as f32 + i as f32 * 0.1, y as f32 + 1.0, 0)).collect();
    let mut p = Player::new(x as f32, y as f32 - 3.0);
    for _ in 0..(120 * TICK_HZ as u32) {
        for z in zs.iter_mut() { z.thumpers = 3; z.tick(&mut p, &mut w); }
    }
    let b = w.barricade((x, y, 0, true)).expect("still standing");
    assert!(b.planks.iter().sum::<i32>() < 4 * PLANK_HEALTH, "the top plank took the hits: {:?}", b.planks);
    assert_eq!(b.planks[..3], [PLANK_HEALTH; 3], "only the last plank nailed on is hit");
    assert_eq!(w.damage_door(x, y, 0, 0), Some(DOOR_HEALTH), "the door itself is untouched behind it");
    assert!(!w.door_open(x, y, 0));
}
