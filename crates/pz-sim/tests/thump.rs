use pz_sim::*;
use pz_world::{flag, World};

fn world() -> Option<World> {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let media = std::path::PathBuf::from(dir).join("media");
    if !media.is_dir() { return None; }
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    Some(w)
}

/// A closed north-edge door with open floor two squares out on both sides.
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
fn thump_timings_match_the_games_animation_data() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let m = std::path::PathBuf::from(dir).join("media");
    if !m.join("anims_X").is_dir() { return; }
    for (i, (xml, clip)) in [("DoorClaw", "Zombie_DoorClaw"), ("door", "Zombie_Door"), ("DoorBang", "Zombie_DoorBang")].iter().enumerate() {
        let len = pz_anim::x_duration(&String::from_utf8_lossy(&std::fs::read(m.join(format!("anims_X/Zombie/{clip}.X"))).unwrap())).unwrap();
        let bang = pz_anim::parse_node(&std::fs::read_to_string(m.join("AnimSets/zombie/thump/DoorBang.xml")).unwrap(), None);
        let node = if *xml == "DoorBang" { bang } else {
            let t = std::fs::read_to_string(m.join(format!("AnimSets/zombie/thump/{xml}.xml"))).unwrap();
            pz_anim::parse_node(&t, Some(&bang))
        };
        let ev = pz_anim::event_times(&node, len, "ThumpFrame");
        let (period, events) = THUMP_ANIMS[i];
        assert_eq!((len / node.speed_scale * TICK_HZ).round() as u32, period, "{xml} loop");
        let ticks: Vec<u32> = ev.iter().map(|e| (e * TICK_HZ).round() as u32).collect();
        assert_eq!(ticks, events, "{xml} events");
    }
}

#[test]
fn one_zombie_cannot_break_a_door_but_a_crowd_can() {
    let Some(mut w) = world() else { return };
    let (x, y) = door(&mut w).expect("a door near Muldraugh");
    let mut player = Player::new(x as f32, y as f32 - 3.0);
    let run = |w: &mut World, crowd: usize| {
        let mut zs: Vec<Zombie> = (0..crowd).map(|i| Zombie::new(x as f32 + i as f32 * 0.1, y as f32 + 1.0, 0)).collect();
        let mut p = Player::new(x as f32, y as f32 - 3.0);
        for _ in 0..(120 * TICK_HZ as u32) {
            for z in zs.iter_mut() { z.thumpers = crowd as u32; z.tick(&mut p, w); }
            if w.door_open(x, y, 0) { return true; }
        }
        false
    };
    let _ = &mut player;
    assert!(!run(&mut w, 1), "a lone zombie only bangs");
    assert!(w.damage_door(x, y, 0, 0) == Some(pz_world::DOOR_HEALTH));
    assert!(run(&mut w, 3), "three zombies break it down");
    assert!(w.door_broken(x, y, 0));
    assert_eq!(w.toggle_door(x, y, 0), None, "a broken door cannot be closed again");
}
