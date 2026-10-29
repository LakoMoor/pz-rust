use pz_sim::*;
use pz_world::World;

#[test]
fn walls_stop_the_player_on_the_real_map() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let media = std::path::PathBuf::from(dir).join("media");
    if !media.is_dir() { return; }
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    // Scan a patch of town: count blocked edges/squares, and sanity-check flags.
    let (mut solid, mut edges) = (0, 0);
    for x in 12600..12700 {
        for y in 1600..1700 {
            let f = w.flags(x, y, 0);
            if f & pz_world::flag::SOLID != 0 { solid += 1; }
            if f & (pz_world::flag::EDGE_N | pz_world::flag::EDGE_W) != 0 { edges += 1; }
        }
    }
    assert!(solid > 0 && edges > 0, "solid={solid} edges={edges}");
    // Walk east along every row: a building wall must stop at least one walker.
    let mut stopped = 0;
    for y in 1600..1700 {
        let mut p = Player::new(12600.0, y as f32);
        for _ in 0..600 { p.tick(Input { dir: [1.0, 0.0], run: true }, &mut w); }
        if p.pos[0] < 12690.0 { stopped += 1; }
    }
    assert!(stopped > 5, "stopped={stopped}");
}
