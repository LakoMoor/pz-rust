use pz_sim::*;
use pz_world::{flag, World};

#[test]
fn closed_doors_block_and_open_doors_pass() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let media = std::path::PathBuf::from(dir).join("media");
    if !media.is_dir() { return; }
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    let mut tested = 0;
    for x in 12600..12700 { for y in 1600..1700 {
        let f = w.flags(x, y, 0);
        if f & flag::DOOR_W != 0 && f & flag::SOLID == 0 && w.flags(x - 1, y, 0) & flag::SOLID == 0 {
            assert!(!w.can_step((x - 1, y), (x, y), 0), "closed door at ({x},{y}) must block");
            let mut p = Player::new(x as f32, y as f32);
            assert!(p.interact(&mut w));
            assert!(w.can_step((x - 1, y), (x, y), 0), "open door at ({x},{y}) must pass");
            tested += 1;
        }
    }}
    assert!(tested > 0, "no doors found");
}
