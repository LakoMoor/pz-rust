//! Map cell files: `.lotheader` ("LOTH") and `world_X_Y.lotpack` ("LOTP").
use crate::{io::Reader, Result};

#[derive(Debug)]
pub struct LotHeader {
    pub version: u32,
    /// Tile names referenced by this cell (indexed by lotpack tile ids).
    pub tile_names: Vec<String>,
    pub rest_offset: usize,
}

impl LotHeader {
    pub fn parse(data: &[u8]) -> Result<LotHeader> {
        let mut r = Reader::new(data);
        r.magic("LOTH")?;
        let version = r.u32()?;
        let n = r.u32()?;
        let tile_names = (0..n).map(|_| r.line()).collect::<Result<_>>()?;
        Ok(LotHeader { version, tile_names, rest_offset: r.pos })
    }
}

#[derive(Debug)]
pub struct LotPack<'a> {
    pub version: u32,
    pub data: &'a [u8],
    /// Absolute offset of each chunk's data (32x32 chunks per cell).
    pub offsets: Vec<u64>,
}

impl<'a> LotPack<'a> {
    pub fn parse(data: &'a [u8]) -> Result<LotPack<'a>> {
        let mut r = Reader::new(data);
        r.magic("LOTP")?;
        let version = r.u32()?;
        let n = r.u32()?;
        let offsets = (0..n).map(|_| r.u64()).collect::<Result<_>>()?;
        Ok(LotPack { version, data, offsets })
    }
}

pub const CHUNK: usize = 8;
pub const SQUARES: usize = CHUNK * CHUNK;

/// One tile column. `tiles` are indices into the header's `tile_names`, bottom-to-top.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Square { pub room: i32, pub tiles: Vec<u32> }

/// All squares of one chunk at one z-level, indexed `y * 8 + x`.
pub type Level = Vec<Square>;

/// Decode a chunk's byte range into z-levels (bottom level first).
///
/// Per square: `n:i32`. `-1` => skip the next `i32` squares; `0` => empty;
/// else `n` ints follow: a room id (`-1` = none) then `n-1` tile indices.
pub fn decode_chunk(data: &[u8]) -> Result<Vec<Level>> {
    let mut r = Reader::new(data);
    let mut levels: Vec<Level> = Vec::new();
    // Skip runs may span level boundaries, so squares are indexed globally.
    let mut i = 0usize;
    while r.pos < data.len() {
        let n = r.i32()?;
        if n == -1 {
            i += r.i32()? as usize;
        } else if n == 0 {
            i += 1;
        } else {
            let room = r.i32()?;
            let tiles = (1..n).map(|_| r.u32()).collect::<Result<_>>()?;
            while levels.len() <= i / SQUARES {
                levels.push(vec![Square { room: -1, tiles: vec![] }; SQUARES]);
            }
            levels[i / SQUARES][i % SQUARES] = Square { room, tiles };
            i += 1;
        }
    }
    if i % SQUARES != 0 { return Err(crate::Error::Invalid(format!("chunk covers {i} squares"))); }
    levels.resize(i / SQUARES, vec![Square { room: -1, tiles: vec![] }; SQUARES]);
    Ok(levels)
}

impl<'a> LotPack<'a> {
    /// Byte range of chunk `i` (row-major, 32x32 per cell).
    pub fn chunk_bytes(&self, i: usize) -> &'a [u8] {
        let start = self.offsets[i] as usize;
        let end = self.offsets.get(i + 1).map_or(self.data.len(), |&o| o as usize);
        &self.data[start..end]
    }
}
