use pz_world::World;
fn main() {
    let media = std::path::PathBuf::from(r"E:\Games\steamapps\common\ProjectZomboid\media");
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    for z in 0..3 { for x in 12552..=12558 {
        w.ensure(x, 1678);
        println!("({x},1678,z{z}) {:?}", w.column(x, 1678, z).map(|c| c.names.iter().map(|s| s.to_string()).collect::<Vec<_>>()));
    }}
}
