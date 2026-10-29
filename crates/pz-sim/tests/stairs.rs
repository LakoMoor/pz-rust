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

/// Staircase found in the real map: bottom step (12556,1678,0), climbing toward -x.
#[test]
fn walking_up_and_down_a_real_staircase() {
    let Some(mut w) = world() else { return };
    let mut p = Player::new(12558.0, 1678.0);
    let west = Input { dir: [-1.0, 0.0], run: false };
    let mut max_h: f32 = 0.0;
    for _ in 0..300 {
        p.tick(west, &mut w);
        max_h = max_h.max(p.height);
        if p.z == 1 { break; }
    }
    assert_eq!(p.z, 1, "did not climb; at {:?}", p.pos);
    assert!(max_h > 0.5, "height should rise smoothly, max {max_h}");
    let east = Input { dir: [1.0, 0.0], run: false };
    for _ in 0..300 { p.tick(east, &mut w); if p.z == 0 { break; } }
    assert_eq!(p.z, 0, "did not descend; at {:?}", p.pos);
}
