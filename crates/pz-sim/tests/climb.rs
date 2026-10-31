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
    // A closed window has to be smashed first (50 glass, 3 per hit), which takes a while.
    for _ in 0..(120 * TICK_HZ as u32) {
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

#[test]
fn a_zombie_playing_dead_ignores_distant_players_and_springs_when_stepped_on() {
    use pz_world::World;
    let mut w = World::new(std::path::PathBuf::from("."));
    let mut z = Zombie::new(0.0, 0.0, 0);
    z.play_dead();
    assert!(z.is_fake_dead());
    let mut far = Player::new(5.0, 0.0);
    for _ in 0..60 { z.tick(&mut far, &mut w); }
    assert!(z.is_fake_dead() && z.pos == [0.0, 0.0], "should lie still while the player is 5 tiles away");
    let mut near = Player::new(1.0, 0.0);
    z.tick(&mut near, &mut w);
    assert!(!z.is_fake_dead());
    for _ in 0..90 { z.tick(&mut near, &mut w); }
    assert!(near.health < 100.0, "the ambusher should have hurt the player");
    // About half of the corpses turn out to be crawlers.
    let crawlers = (0..400).filter(|i| { let mut z = Zombie::new(*i as f32 * 3.1, 1.0, 0); z.play_dead(); z.is_crawler() }).count();
    assert!((140..260).contains(&crawlers), "{crawlers}");
}

#[test]
fn idle_zombies_wander_more_often_in_the_rain() {
    use pz_sim::Weather;
    let mut w = pz_world::World::new(std::path::PathBuf::from("."));
    let mut count = |rain: f32| {
        let mut moved = 0;
        for i in 0..40 {
            let mut z = Zombie::new(i as f32 * 5.0, 0.0, 0);
            let mut far = Player::new(900.0, 900.0);
            let weather = Weather { rain, ..Weather::default() };
            for _ in 0..1500 { z.tick_in(&mut far, &mut w, &weather); }
            if z.pos != [i as f32 * 5.0, 0.0] { moved += 1; }
        }
        moved
    };
    let (dry, wet) = (count(0.0), count(1.0));
    assert!(wet >= dry, "wet {wet} dry {dry}");
}

#[test]
fn a_zombie_needs_the_get_up_animation_before_it_moves_again() {
    let mut w = pz_world::World::new(std::path::PathBuf::from("."));
    let mut z = Zombie::new(0.0, 0.0, 0);
    z.on_floor = true;
    z.reanimate_timer = 1.0;
    let mut p = Player::new(8.0, 0.0);
    let mut first_move = None;
    for t in 0..400 {
        z.tick(&mut p, &mut w);
        if first_move.is_none() && z.pos[0] > 0.01 { first_move = Some(t); }
    }
    let t = first_move.expect("it gets up and walks");
    assert!((GET_UP_TICKS..GET_UP_TICKS + 5).contains(&(t as u32)), "first moved at tick {t}");
}

#[test]
fn get_up_ticks_match_the_games_animation_data() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let m = std::path::PathBuf::from(dir).join("media");
    let Ok(x) = std::fs::read(m.join("anims_X/Zombie/Zombie_GetUp.X")) else { return };
    let len = pz_anim::x_duration(&String::from_utf8_lossy(&x)).unwrap();
    let node = pz_anim::parse_node(&std::fs::read_to_string(m.join("AnimSets/zombie/getup-fromOnBack/getUpFromOnBack.xml")).unwrap(), None);
    assert_eq!(node.anim_name, "Zombie_GetUp");
    assert_eq!((len / node.speed_scale * TICK_HZ).round() as u32, GET_UP_TICKS);
}

#[test]
fn fall_down_ticks_match_the_games_animation_data() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let m = std::path::PathBuf::from(dir).join("media");
    let Ok(x) = std::fs::read(m.join("anims_X/Zombie/Zombie_Death.X")) else { return };
    let len = pz_anim::x_duration(&String::from_utf8_lossy(&x)).unwrap();
    let node = pz_anim::parse_node(&std::fs::read_to_string(m.join("AnimSets/zombie/falldown/default.xml")).unwrap(), None);
    assert_eq!(node.anim_name, "Zombie_Death");
    assert_eq!((len / node.speed_scale * TICK_HZ).round() as u32, FALL_DOWN_TICKS);
}

#[test]
fn zombies_smash_closed_windows_one_hit_at_a_time_before_climbing() {
    let Some(mut w) = world() else { return };
    let (a, b) = hurdle_pair(&mut w).expect("a climbable edge near Muldraugh");
    let Some(key) = w.intact_window_between(a, b, 0) else { return }; // this edge is a fence, not a window
    let mut player = Player::new(b.0 as f32 + 1.5, b.1 as f32);
    let mut z = Zombie::new(a.0 as f32, a.1 as f32, 0);
    let (mut hits_seen, mut climbed_before_smash) = (0, false);
    let mut last = pz_world::WINDOW_HEALTH;
    for _ in 0..(120 * TICK_HZ as u32) {
        z.tick(&mut player, &mut w);
        let left = if w.window_smashed(key) { 0 } else { w.damage_window(key, 0) };
        if left < last { hits_seen += 1; assert!(last - left == 3 || left == 0, "strength 3 per hit"); last = left; }
        if z.is_climbing() && !w.window_smashed(key) { climbed_before_smash = true; }
        if w.window_smashed(key) { break; }
    }
    assert!(w.window_smashed(key) && !climbed_before_smash, "smashed {} hits {hits_seen}", w.window_smashed(key));
    assert!(hits_seen >= 16, "{hits_seen}");
}
