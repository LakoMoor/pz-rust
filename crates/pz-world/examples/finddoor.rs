//! Print a few closed doors with open ground in front of them (handy for screenshots): `finddoor [x0 y0]`.
use pz_world::{flag, World};
fn main() {
    let pz = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let media = std::path::PathBuf::from(pz).join("media");
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    let a: Vec<i32> = std::env::args().skip(1).filter_map(|s| s.parse().ok()).collect();
    let (x0, y0) = (a.first().copied().unwrap_or(12600), a.get(1).copied().unwrap_or(1600));
    let mut n = 0;
    for y in y0..y0 + 80 { for x in x0..x0 + 80 {
        if w.flags(x, y, 0) & flag::DOOR_N != 0 { println!("{x},{y}"); n += 1; if n == 8 { return; } }
    } }
}
