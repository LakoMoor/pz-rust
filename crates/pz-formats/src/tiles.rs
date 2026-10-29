//! `.tiles` tile definition files ("tdef"): tilesets and per-tile properties.
use crate::{io::Reader, Result};

#[derive(Debug, Clone, Default)]
pub struct Tile { pub props: Vec<(String, String)> }

#[derive(Debug, Clone)]
pub struct TileSet {
    pub name: String,
    pub image: String,
    pub cols: u32,
    pub rows: u32,
    pub id: u32,
    pub tiles: Vec<Tile>,
}

#[derive(Debug)]
pub struct TileDefs { pub version: u32, pub sets: Vec<TileSet> }

impl TileDefs {
    pub fn parse(data: &[u8]) -> Result<TileDefs> {
        let mut r = Reader::new(data);
        r.magic("tdef")?;
        let version = r.u32()?;
        let n = r.u32()?;
        let mut sets = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let name = r.line()?;
            let image = r.line()?;
            let (cols, rows, id, count) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
            let mut tiles = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let np = r.u32()?;
                let mut props = Vec::with_capacity(np as usize);
                for _ in 0..np { props.push((r.line()?, r.line()?)); }
                tiles.push(Tile { props });
            }
            sets.push(TileSet { name, image, cols, rows, id, tiles });
        }
        Ok(TileDefs { version, sets })
    }
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<TileDefs> { Self::parse(&std::fs::read(path)?) }
}
