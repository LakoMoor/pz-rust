use pz_world::World;
fn main() {
    let media = std::path::PathBuf::from(r"E:\Games\steamapps\common\ProjectZomboid\media");
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    w.ensure(12611, 1623);
    let names: Vec<String> = w.column(12611, 1623, 0).unwrap().names.iter().map(|s| s.to_string()).collect();
    for n in &names { println!("{n}: door={}", w.is_door_tile(n)); }
    println!("flags {:#x}", w.flags(12611, 1623, 0));
    w.toggle_door(12611, 1623, 0);
    println!("after {:#x}", w.flags(12611, 1623, 0));
}
