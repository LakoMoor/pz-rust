use pz_formats::lot::*;
fn main() {
    let dir = r"E:\Games\steamapps\common\ProjectZomboid\media\maps\Muldraugh, KY";
    let hdr = std::fs::read(format!("{dir}/49_6.lotheader")).unwrap();
    let h = LotHeader::parse(&hdr).unwrap();
    println!("hdr v{} tiles={} rest@{}", h.version, h.tile_names.len(), h.rest_offset);
    let d = std::fs::read(format!("{dir}/world_49_6.lotpack")).unwrap();
    let p = LotPack::parse(&d).unwrap();
    let mut sizes: Vec<(u64, usize)> = p.offsets.iter().enumerate().map(|(i, &o)| {
        let end = p.offsets.get(i + 1).copied().unwrap_or(d.len() as u64);
        (end - o, i)
    }).collect();
    sizes.sort();
    println!("sizes: min {:?} max {:?}", sizes[0], sizes[sizes.len() - 1]);
    let (sz, i) = sizes[sizes.len() - 1];
    let o = p.offsets[i] as usize;
    println!("chunk {i} size {sz}: {:02x?}", &d[o..o + (sz as usize).min(400)]);
}
