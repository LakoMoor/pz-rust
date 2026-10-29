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

/// Find a pair of squares on a row with a wall between them that A* can still route around.
fn walled_pair(w: &mut World) -> Option<((i32, i32), (i32, i32), usize)> {
    for y in 1600..1700 {
        for x in 12600..12690 {
            let (a, b) = ((x, y), (x + 6, y));
            if !w.has_floor(a.0, a.1, 0) || !w.has_floor(b.0, b.1, 0) { continue; }
            let straight_blocked = (0..6).any(|i| !w.can_step((x + i, y), (x + i + 1, y), 0));
            if !straight_blocked { continue; }
            if let Some(p) = w.find_path(a, b, 0, 4000) {
                if p.len() > 6 { return Some((a, b, p.len())); }
            }
        }
    }
    None
}

#[test]
fn astar_routes_around_walls_on_the_real_map() {
    let Some(mut w) = world() else { return };
    let (a, b, len) = walled_pair(&mut w).expect("a walled pair with a detour");
    let path = w.find_path(a, b, 0, 4000).unwrap();
    assert_eq!(path.len(), len);
    assert_eq!(*path.last().unwrap(), b);
    // Every step is legal and orthogonal.
    let mut cur = a;
    for &n in &path {
        assert_eq!((n.0 - cur.0).abs() + (n.1 - cur.1).abs(), 1);
        assert!(w.can_step(cur, n, 0), "illegal step {cur:?} -> {n:?}");
        cur = n;
    }
    // Same square: empty path; an enclosed target far away within a tiny budget: gives up.
    assert_eq!(w.find_path(a, a, 0, 10), Some(vec![]));
    assert_eq!(w.find_path(a, b, 0, 1), None);
}

#[test]
fn zombies_walk_around_a_wall_to_reach_the_player() {
    let Some(mut w) = world() else { return };
    let (a, b, _) = walled_pair(&mut w).expect("a walled pair with a detour");
    let mut player = Player::new(b.0 as f32, b.1 as f32);
    let mut z = Zombie::new(a.0 as f32, a.1 as f32, 0);
    let mut reached = false;
    for _ in 0..(60 * TICK_HZ as u32) {
        z.tick(&mut player, &mut w);
        let d = (z.pos[0] - player.pos[0]).hypot(z.pos[1] - player.pos[1]);
        if d < 1.0 { reached = true; break; }
    }
    assert!(reached, "zombie stuck at {:?} while heading to {:?}", z.pos, player.pos);
}
