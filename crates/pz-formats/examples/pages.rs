use pz_formats::pack::Pack;
fn main() {
    let dir = r"E:\Games\steamapps\common\ProjectZomboid\media\texturepacks";
    for n in ["Tiles2x.floor.pack","Tiles2x.pack","Overlays2x.floor.pack","Overlays2x.pack","B42ChunkCaching2x.floor.pack","B42ChunkCaching2x.pack"] {
        let p = Pack::open(format!("{dir}/{n}")).unwrap();
        let mut sizes = std::collections::BTreeMap::new();
        for pg in &p.pages { let w = u32::from_be_bytes(pg.png[16..20].try_into().unwrap()); let h = u32::from_be_bytes(pg.png[20..24].try_into().unwrap()); *sizes.entry((w,h)).or_insert(0) += 1; }
        println!("{n}: {} pages {:?}", p.pages.len(), sizes);
    }
}
