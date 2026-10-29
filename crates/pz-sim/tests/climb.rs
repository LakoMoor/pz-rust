use pz_sim::*;
use pz_world::World;

fn world() -> Option<World> {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let media = std::path::PathBuf::from(dir).join("media");
    if !media.is_dir() { return None; }
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    Some(w)
}

/// A square pair (a, b) on the x axis separated by a climbable edge, with open floor around.
fn hurdle_pair(w: &mut World) -> Option<((i32, i32), (i32, i32))> {
    for y in 1500..1800 {
        for x in 12500..12800 {
            let (a, b) = ((x, y), (x + 1, y));
            if !w.has_floor(a.0, a.1, 0) || !w.has_floor(b.0, b.1, 0) { continue; }
            if !w.can_step(a, b, 0) && w.hurdle(a, b, 0) && w.can_step(b, (x + 2, y), 0) {
                return Some((a, b));
            }
        }
    }
    None
}

#[test]
fn hurdle_detects_windows_and_fences_on_the_real_map() {
    let Some(mut w) = world() else { return };
    let (a, b) = hurdle_pair(&mut w).expect("a climbable edge near Muldraugh");
    assert!(!w.can_step(a, b, 0));
    assert!(w.hurdle(a, b, 0));
    // The same edge is climbable in the other direction too, and a plain open step is not a hurdle.
    assert!(w.hurdle(b, a, 0));
    assert!(!w.hurdle(b, (b.0 + 1, b.1), 0));
}

#[test]
fn zombie_climbs_over_a_hurdle_to_reach_the_player() {
    let Some(mut w) = world() else { return };
    let (a, b) = hurdle_pair(&mut w).expect("a climbable edge near Muldraugh");
    let mut player = Player::new(b.0 as f32 + 1.5, b.1 as f32);
    let mut z = Zombie::new(a.0 as f32, a.1 as f32, 0);
    let (mut climbed, mut crossed) = (false, false);
    for _ in 0..(20 * TICK_HZ as u32) {
        z.tick(&mut player, &mut w);
        climbed |= z.is_climbing();
        if z.pos[0] > b.0 as f32 - 0.4 { crossed = true; break; }
    }
    assert!(climbed, "zombie never started climbing, at {:?}", z.pos);
    assert!(crossed, "zombie did not get across, at {:?}", z.pos);
}

#[test]
fn crawlers_do_not_climb() {
    let Some(mut w) = world() else { return };
    let (a, b) = hurdle_pair(&mut w).expect("a climbable edge near Muldraugh");
    let mut player = Player::new(b.0 as f32 + 1.5, b.1 as f32);
    let mut z = Zombie::new(a.0 as f32, a.1 as f32, 0);
    z.set_crawler();
    for _ in 0..(10 * TICK_HZ as u32) { z.tick(&mut player, &mut w); }
    assert!(!z.is_climbing() && z.pos[0] < b.0 as f32 - 0.4, "crawler crossed to {:?}", z.pos);
}
