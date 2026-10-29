use pz_formats::tiles::TileDefs;
fn main() {
    let want: Vec<String> = std::env::args().skip(1).collect();
    let d = TileDefs::open(r"E:\Games\steamapps\common\ProjectZomboid\media\newtiledefinitions.tiles").unwrap();
    for s in &d.sets { for (i, t) in s.tiles.iter().enumerate() {
        let n = format!("{}_{}", s.name, i);
        if want.contains(&n) { println!("{n}: {:?}", t.props.iter().filter(|(_, v)| true || !v.is_empty()).map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>()); }
    }}
}
