//! Differential checks against the user's installed game. Skipped if PZ is absent
//! (set PZ_DIR to override the location).
use pz_formats::{pack::Pack, tiles::TileDefs};
use std::path::PathBuf;

fn media() -> Option<PathBuf> {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let p = PathBuf::from(dir).join("media");
    p.is_dir().then_some(p)
}

#[test]
fn packs_parse_and_entries_fit_pages() {
    let Some(m) = media() else { return };
    for name in ["Clock2x.pack", "Overlays2x.floor.pack", "Tiles2x.floor.pack"] {
        let pack = Pack::open(m.join("texturepacks").join(name)).unwrap();
        for page in &pack.pages {
            let (w, h, rgba) = pz_formats::pack::decode_png_rgba(&page.png).unwrap();
            assert_eq!(rgba.len(), (w * h * 4) as usize);
            for e in &page.entries {
                assert!(e.x + e.w <= w && e.y + e.h <= h, "{} out of page {}", e.name, page.name);
            }
        }
    }
}

/// The game ships `.tiles` (binary) and `.tiles.txt` (text): use the text as an oracle.
#[test]
fn tiles_binary_matches_text() {
    let Some(m) = media() else { return };
    let bin = TileDefs::open(m.join("newtiledefinitions.tiles")).unwrap();
    let txt = std::fs::read_to_string(m.join("newtiledefinitions.tiles.txt")).unwrap();
    let text_sets = txt.matches("tileset\n").count() + txt.matches("tileset\r\n").count();
    let text_tiles = txt.lines().filter(|l| l.trim() == "tile").count();
    assert_eq!(bin.sets.len(), text_sets);
    assert_eq!(bin.sets.iter().flat_map(|s| &s.tiles).filter(|t| !t.props.is_empty()).count(), text_tiles);
    let first = &bin.sets[0];
    assert_eq!(first.name, "advertising_01");
    assert_eq!((first.cols, first.rows, first.id), (8, 16, 88));
}

/// Every chunk of every Muldraugh cell must decode with its bytes consumed exactly
/// and all tile ids inside the header's name table.
#[test]
fn lotpack_chunks_decode_exactly() {
    use pz_formats::lot::*;
    let Some(m) = media() else { return };
    let dir = m.join("maps").join("Muldraugh, KY");
    let (mut cells, mut chunks) = (0, 0);
    for ent in std::fs::read_dir(&dir).unwrap() {
        let path = ent.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some(cell) = name.strip_suffix(".lotheader") else { continue };
        let hdr = LotHeader::parse(&std::fs::read(&path).unwrap()).unwrap();
        let pack_bytes = std::fs::read(dir.join(format!("world_{cell}.lotpack"))).unwrap();
        let pack = LotPack::parse(&pack_bytes).unwrap();
        assert_eq!(pack.offsets.len(), 1024);
        for i in 0..1024 {
            let levels = decode_chunk(pack.chunk_bytes(i)).unwrap_or_else(|e| panic!("{cell} chunk {i}: {e}"));
            for t in levels.iter().flatten().flat_map(|s| &s.tiles) {
                assert!((*t as usize) < hdr.tile_names.len(), "{cell} chunk {i}: tile {t}");
            }
            chunks += 1;
        }
        cells += 1;
    }
    assert!(cells > 1000, "cells={cells}");
    eprintln!("decoded {cells} cells, {chunks} chunks");
}
