use pz_world::{flag::*, World};
fn main() {
    let media = std::path::PathBuf::from(r"E:\Games\steamapps\common\ProjectZomboid\media");
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    let mut shown = 0;
    for x in 12544..12800 { for y in 1536..1792 {
        for z in 0..3 {
            let f = w.flags(x, y, z);
            if f & (STAIR_BN | STAIR_BW) != 0 && shown < 6 {
                shown += 1;
                println!("B at ({x},{y},{z}) f={f:#x}");
                for d in -3..=3i32 {
                    println!("  N-line y{:+}: {:#x}   W-line x{:+}: {:#x}", d, w.flags(x, y + d, z), d, w.flags(x + d, y, z));
                }
            }
        }
    }}
}
