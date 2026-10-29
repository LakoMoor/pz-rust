use pz_formats::pack::{decode_png_rgba, Pack};
use std::{collections::HashMap, path::Path};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PageKey { pub pack: u16, pub page: u16 }

#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub page: PageKey,
    pub x: u32, pub y: u32, pub w: u32, pub h: u32,
    pub ox: u32, pub oy: u32, pub ow: u32, pub oh: u32,
}

/// Name -> sprite index over the 2x texture packs; page pixels are decoded on demand.
pub struct Assets { packs: Vec<Pack>, index: HashMap<String, Sprite> }

const PACKS: &[&str] = &[
    "Tiles2x.floor.pack", "Tiles2x.pack", "Overlays2x.floor.pack", "Overlays2x.pack",
    "B42ChunkCaching2x.floor.pack", "B42ChunkCaching2x.pack",
];

impl Assets {
    pub fn load(media: &Path) -> pz_formats::Result<Assets> {
        let mut packs = Vec::new();
        let mut index = HashMap::new();
        for name in PACKS {
            let Ok(pack) = Pack::open(media.join("texturepacks").join(name)) else { continue };
            let pi = packs.len() as u16;
            for (gi, page) in pack.pages.iter().enumerate() {
                for e in &page.entries {
                    index.entry(e.name.clone()).or_insert(Sprite {
                        page: PageKey { pack: pi, page: gi as u16 },
                        x: e.x, y: e.y, w: e.w, h: e.h, ox: e.ox, oy: e.oy, ow: e.ow, oh: e.oh,
                    });
                }
            }
            packs.push(pack);
        }
        Ok(Assets { packs, index })
    }
    pub fn sprite(&self, name: &str) -> Option<&Sprite> { self.index.get(name) }
    pub fn decode_page(&self, k: PageKey) -> pz_formats::Result<(u32, u32, Vec<u8>)> {
        decode_png_rgba(&self.packs[k.pack as usize].pages[k.page as usize].png)
    }
    pub fn sprite_count(&self) -> usize { self.index.len() }
}
